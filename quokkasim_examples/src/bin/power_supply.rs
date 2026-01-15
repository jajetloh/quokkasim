use quokkasim::prelude::*;
use core::f64;
use std::time::Duration;

struct Interpolator {
    points: Vec<(Duration, f64)>,
    period: Option<Duration>,
}

impl Interpolator {
    pub fn new(mut pts: Vec<(Duration, f64)>, period: Option<Duration>) -> Self {
        // sort by time ascending
        pts.sort_by_key(|&(t, _)| t);
        assert!(!pts.is_empty(), "need at least one sample");
        Interpolator { points: pts, period }
    }

    pub fn get(&self, t: Duration) -> f64 {
        if let Some(period) = self.period {
            // Work in u128 to avoid overflow even for very large Durations
            let t_ns = t.as_secs()  as u128 * 1_000_000_000 + t.subsec_nanos()  as u128;
            let period_ns = period.as_secs()  as u128 * 1_000_000_000 + period.subsec_nanos()  as u128;

            let rem_ns = t_ns % period_ns;

            // split back into secs + nanos
            let secs = (rem_ns / 1_000_000_000) as u64;
            let nanos = (rem_ns % 1_000_000_000) as u32;
            interp(&self.points, Duration::new(secs, nanos))
        } else {
            interp(&self.points, t)
        }
    }
}

fn interp(points: &[(Duration, f64)], t: Duration) -> f64 {
    assert!(
        !points.is_empty(),
        "need at least one (Duration, f64) point"
    );

    // If there's only one sample, just return it:
    if points.len() == 1 {
        return points[0].1;
    }

    match points.binary_search_by_key(&t, |&(d, _)| d) {
        // exact match
        Ok(idx) => points[idx].1,

        // t would be inserted at index 0 → before the first point → clamp
        Err(0) => points[0].1,

        // t would be inserted at len → after the last point → clamp
        Err(i) if i >= points.len() => points.last().unwrap().1,

        // between points[i-1] and points[i]
        Err(i) => {
            let (t0, v0) = points[i - 1];
            let (t1, v1) = points[i];

            // convert Durations to f64 seconds
            let dt_total = (t1 - t0).as_secs_f64();
            let dt_here = (t - t0).as_secs_f64();

            let alpha = dt_here / dt_total;
            v0 + alpha * (v1 - v0)
        }
    }
}

// Power source from solar energy
#[derive(WithMethods)]
struct SolarSupply {
    // Identification
    pub element_name: String,
    pub element_code: String,
    pub element_type: String,

    // Ports
    pub req_output_state: Requestor<(), ContStockState>,
    pub push_downstream: Output<(f64, EventMetadata)>,

    pub log_emitter: Output<ContProcessLog<f64>>,

    // Configuration
    pub output_time_series_base_rate: Interpolator,
    pub output_time_series_variability: Distribution,
    pub update_period: Duration,

    // Runtime state
    pub process_state: Option<(Duration, f64)>,
    
    // Internals
    pub time_to_next_process_event: Option<Duration>,
    pub previous_check_time: MonotonicTime,
    pub scheduled_event: Option<(MonotonicTime, ActionKey)>,
    pub previous_event: EventMetadata,
}

impl Default for SolarSupply {
    fn default() -> Self {
        SolarSupply {
            element_name: "Solar Power Supply".to_string(),
            element_code: "SPS-001".to_string(),
            element_type: "PowerSupply".to_string(),

            req_output_state: Requestor::new(),
            push_downstream: Output::new(),

            log_emitter: Output::new(),

            output_time_series_base_rate: Interpolator::new(
                vec![
                    (Duration::from_secs(0), 0.0),
                ],
                None,
            ),
            output_time_series_variability: Distribution::Constant(1.0),
            update_period: Duration::from_secs(60),

            process_state: None,
            time_to_next_process_event: Some(Duration::from_secs(0)),
            
            previous_check_time: MonotonicTime::EPOCH,
            scheduled_event: None,
            previous_event: EventMetadata::default(),
        }
    }
}

impl Model for SolarSupply {
    fn init(mut self, cs: &mut Context<Self>) -> impl Future<Output=InitializedModel<Self>> {
        async move {
            let source_event_id = EventMetadata::from_init();
            self.update_state(source_event_id, cs).await;
            println!("Initialized Splitter {}", self.element_name);
            self.into()
        }
    }
}

impl ContProcessCore for SolarSupply {
fn update_state(
        &mut self,
        source_event_id: EventMetadata,
        cx: &mut Context<Self>,
    ) -> impl Future<Output = ()> {
        async move {
            self.update_state_since_last_update(&mut source_event_id.clone(), cx)
                .await;
            self.update_state_decision_logic(&mut source_event_id.clone(), cx)
                .await;
            self.update_state_for_next_event(&mut source_event_id.clone(), cx)
                .await;
        }
    }

    fn element_name(&self) -> &str {
        &self.element_name
    }
    fn element_code(&self) -> &str {
        &self.element_code
    }
    fn element_type(&self) -> &str {
        &self.element_type
    }
    fn get_next_event_meta(&mut self) -> EventMetadata {
        self.previous_event.next()
    }
    fn scheduled_event(&mut self) -> &mut Option<(MonotonicTime, ActionKey)> {
        &mut self.scheduled_event
    }
    fn previous_check_time(&mut self) -> &mut MonotonicTime {
        &mut self.previous_check_time
    }
    fn time_to_next_process_event(&mut self) -> &mut Option<Duration> {
        &mut self.time_to_next_process_event
    }
}

impl ContProcessUpdateSinceLast<f64> for SolarSupply
where
    Self: ContProcessCore,
{
    fn update_process_state_since_prev_event(
        &mut self, source_event_id: &mut EventMetadata,
        cx: &mut Context<Self>,
        duration_since_prev: Duration
    ) -> impl Future<Output = ()> {
        async move {
            if let Some((mut time_left, resource)) = self.process_state.take() {
                time_left = time_left.saturating_sub(duration_since_prev);
                if time_left.is_zero() {
                    let log_payload = resource.clone();
                    *source_event_id = self.log_type_process_success(source_event_id, resource.total(), log_payload, cx).await;
                    self.push_downstream
                        .send((resource, source_event_id.clone()))
                        .await;
                    self.process_state = None;
                } else {
                    self.process_state = Some((time_left, resource));
                }
            }
        }
    }

    fn log_type_process_success(&mut self, source_event_id: &mut EventMetadata, quantity: f64, resource: f64, cx: &mut Context<Self>) -> impl Future<Output = EventMetadata> {
        async move {
            let current_event = self.get_next_event_meta();
            self.log_emitter.send(ContProcessLog {
                time: cx.time().to_chrono_date_time(0).unwrap().to_string(),
                event_id: current_event.clone(),
                source_event_id: source_event_id.clone(),
                element_name: self.element_name.clone(),
                element_type: self.element_type.clone(),
                details: DefaultContProcessLogType::ProcessSuccess { quantity, resource },
            }).await;
            current_event
        }
    }
}

impl ContProcessUpdateDecisionLogic<
    f64,
> for SolarSupply
where   
    Self: ContProcessCore,
{
    fn update_state_decision_logic(
            &mut self,
            source_event_id: &mut EventMetadata,
            cx: &mut Context<Self>,
        ) -> impl Future<Output = ()> {
        async move {
            match self.process_state {
                None => {
                    let ds_state = self.req_output_state.send(()).await.next();

                    match &ds_state {
                        Some(ContStockState::Empty { .. })
                        | Some(ContStockState::Normal { .. }) => {
                            let midnight_reference = MonotonicTime::try_from_date_time(2025, 1, 1, 0, 0, 0, 0).unwrap();
                            let time_since_ref = cx.time().duration_since(midnight_reference);
                            let process_quantity = self.output_time_series_base_rate.get(time_since_ref) * self.output_time_series_variability.sample() * self.update_period.as_secs_f64();

                            *source_event_id = self.log_type_withdraw_request(source_event_id, process_quantity, cx).await;
                            self.process_state = Some((
                                self.update_period.clone(),
                                process_quantity
                            ));
                            *source_event_id = self.log_type_process_start(source_event_id, process_quantity, process_quantity, cx).await;
                            self.time_to_next_process_event = Some(self.update_period.clone());
                        }
                        None => {
                            *source_event_id = self.log_type_process_failure(source_event_id, "Downstream is not connected", cx).await;
                            self.time_to_next_process_event = None;
                        }
                        Some(ContStockState::Full { .. }) => {
                            *source_event_id = self.log_type_process_failure(source_event_id, "Downstream is full", cx).await;
                            self.time_to_next_process_event = None;
                        }
                    }
                }
                Some((time, _)) => {
                    self.time_to_next_process_event = Some(time);
                }
            }
        }
    }

    
    fn log_type_withdraw_request(&mut self, source_event_id: &mut EventMetadata, quantity: f64, cx: &mut Context<Self>) -> impl Future<Output = EventMetadata> {
        async move {
            let current_event = self.get_next_event_meta();
            self.log_emitter.send(ContProcessLog {
                time: cx.time().to_chrono_date_time(0).unwrap().to_string(),
                event_id: current_event.clone(),
                source_event_id: source_event_id.clone(),
                element_name: self.element_name.clone(),
                element_type: self.element_type.clone(),
                details: DefaultContProcessLogType::WithdrawRequest { quantity },
            }).await;
            current_event
        }
    }

    fn log_type_process_start(&mut self, source_event_id: &mut EventMetadata, quantity: f64, resource: f64, cx: &mut Context<Self>) -> impl Future<Output = EventMetadata> {
        async move {
            let current_event = self.get_next_event_meta();
            self.log_emitter.send(ContProcessLog {
                time: cx.time().to_chrono_date_time(0).unwrap().to_string(),
                event_id: current_event.clone(),
                source_event_id: source_event_id.clone(),
                element_name: self.element_name.clone(),
                element_type: self.element_type.clone(),
                details: DefaultContProcessLogType::ProcessStart { quantity, resource },
            }).await;
            current_event
        }
    }

    fn log_type_process_failure(&mut self, source_event_id: &mut EventMetadata, reason: &'static str, cx: &mut Context<Self>) -> impl Future<Output = EventMetadata> {
        async move {
            let current_event = self.get_next_event_meta();
            self.log_emitter.send(ContProcessLog {
                time: cx.time().to_chrono_date_time(0).unwrap().to_string(),
                event_id: current_event.clone(),
                source_event_id: source_event_id.clone(),
                element_name: self.element_name.clone(),
                element_type: self.element_type.clone(),
                details: DefaultContProcessLogType::ProcessFailure { reason },
            }).await;
            current_event
        }
    }
}

impl ContProcessUpdateForNextEvent for SolarSupply
where
    Self: ContProcessCore,
{}

impl Connect<SolarSupply, DefaultContStock<f64, ContStockState, ContStockLog<f64>>> for Connection {
    fn connect(
        &mut self,
        a: (&mut SolarSupply, &Address<SolarSupply>, Option<usize>),
        b: (
            &mut DefaultContStock<f64, ContStockState, ContStockLog<f64>>,
            &Address<DefaultContStock<f64, ContStockState, ContStockLog<f64>>>,
            Option<usize>
        )
    ) -> Result<(), Box<dyn std::error::Error>> {
        a.0.req_output_state.connect(DefaultContStock::get_state_async, b.1.clone());
        a.0.push_downstream.connect(DefaultContStock::add, b.1.clone());
        b.0.state_emitter.connect(SolarSupply::update_state, a.1.clone());
        Ok(())
    }
}

fn main() {
    let mut solar_source = SolarSupply::default();
    solar_source.output_time_series_base_rate = Interpolator { points: vec![
        (Duration::from_secs(0), 0.0),
        (Duration::from_secs(6*3600), 0.0),
        (Duration::from_secs(7*3600), 2.0),
        (Duration::from_secs(12*3600), 5.0),
        (Duration::from_secs(17*3600), 2.0),
        (Duration::from_secs(18*3600), 0.0),
        (Duration::from_secs(24*3600), 0.0),
    ], period: Some(Duration::from_secs(24*3600)) };
    solar_source.update_period = Duration::from_secs(300);
    let solar_mbox = Mailbox::new();
    let solar_address = solar_mbox.address();

    let mut battery_storage = DefaultContStock::<f64, ContStockState, ContStockLog<f64>>::default();
    battery_storage.max_capacity = f64::INFINITY;
    let battery_mbox = Mailbox::new();
    let battery_address = battery_mbox.address();

    let process_logger = EventQueue::<ContProcessLog<f64>>::default();
    let stock_logger = EventQueue::<ContStockLog<f64>>::default();

    solar_source.log_emitter.connect_sink(&process_logger);
    battery_storage.log_emitter.connect_sink(&stock_logger);

    let mut connection = Connection {};

    connection.connect(
        (&mut solar_source, &solar_address, None),
        (&mut battery_storage, &battery_address, None),
    ).unwrap();

    let mut sim_init = SimInit::new().add_model(solar_source, solar_mbox, "solar_source")
        .add_model(battery_storage, battery_mbox, "battery_storage");

    let start_time = MonotonicTime::try_from_date_time(2025, 1, 1, 0, 0, 0, 0).unwrap();
    let sim_duration = Duration::from_secs(24 * 60 * 60); // 24 hours
    let (mut sim, mut sched) = sim_init.init(start_time.clone()).unwrap();

    sim.step_until(start_time + sim_duration).unwrap();

    for log in process_logger.into_reader() {
        println!("{:?}", log);
    }

    for log in stock_logger.into_reader() {
        println!("{:?}", log);
    }
}
