use std::{collections::HashMap, error::Error, fs::create_dir_all, time::Duration};
use quokkasim::{define_model_enums, prelude::*};
use serde::Serialize;
use std::fmt::Debug;

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize)]
enum CarJob {
    ReplaceTyres,
    ChangeOil,
    ReplaceBrakes,
}

#[derive(Clone, Debug, Default, Serialize)]
struct Car {
    id: usize,
    jobs: Vec<CarJob>,
}

struct IncomingCarFactory {
    next_index: usize,
    rng: Distribution,
}

impl Default for IncomingCarFactory {
    fn default() -> Self {
        Self::new(&mut DistributionFactory::new(1))
    }
}

impl IncomingCarFactory {
    fn new(df: &mut DistributionFactory) -> Self {
        Self {
            next_index: 0,
            rng: df.create(DistributionConfig::Uniform { min: 0., max: 1. }).unwrap(),
        }
    }
}

impl ItemFactory<Car> for IncomingCarFactory {
    fn create_item(&mut self) -> Car {
        let id = self.next_index;
        self.next_index += 1;

        // Assign jobs based on static probabilities
        let n = self.rng.sample();
        let jobs: Vec<CarJob> = match n {
            n if n < 0. => panic!("Random number out of bounds"),
            n if n <= 0.1 => vec![CarJob::ReplaceTyres],
            n if n <= 0.2 => vec![CarJob::ChangeOil],
            n if n <= 0.4 => vec![CarJob::ChangeOil, CarJob::ReplaceTyres],
            n if n <= 0.7 => vec![CarJob::ReplaceBrakes],
            n if n <= 1. => vec![CarJob::ChangeOil, CarJob::ReplaceTyres, CarJob::ReplaceBrakes],
            _ => panic!("Random number out of bounds"),
        };
        Car { id, jobs }
    }
}

type Worker = String;

#[derive(WithMethods)]
struct CarHoistProcess {
    // Identification
    pub element_name: String,
    pub element_code: String,
    pub element_type: String,

    // Ports
    pub req_cars_ready: Requestor<(), DiscreteStockState>,
    pub withdraw_car: Requestor<((), EventId), Option<Car>>,
    pub push_car: Output<(Car, EventId)>,

    pub req_workers: Requestor<(), DiscreteStockState>,
    pub withdraw_worker: Requestor<((), EventId), Option<Worker>>,
    pub push_worker: Output<(Worker, EventId)>,

    pub req_environment: Requestor<(), BasicEnvironmentState>,
    pub log_emitter: Output<DiscreteProcessLog<(Worker, Car)>>,

    // Configuration
    pub job_duration_distrs: HashMap<CarJob, Distribution>,
    pub delay_modes: DelayModes,

    // Runtime state
    pub process_state: Option<(Duration, (Worker, Car))>,
    pub env_state: BasicEnvironmentState,

    // Internals
    time_to_next_process_event: Option<Duration>,
    time_to_next_delay_event: Option<Duration>,
    scheduled_event: Option<(MonotonicTime, ActionKey)>,
    next_event_index: u64,
    previous_check_time: MonotonicTime,
}

impl Default for CarHoistProcess {
    fn default() -> Self {
        Self {
            element_name: "Car Hoist".into(),
            element_code: "P1".into(),
            element_type: "CarHoistProcess".into(),
            req_cars_ready: Requestor::new(),
            withdraw_car: Requestor::new(),
            push_car: Output::new(),
            req_workers: Requestor::new(),
            withdraw_worker: Requestor::new(),
            push_worker: Output::new(),
            log_emitter: Output::new(),
            req_environment: Requestor::new(),
            job_duration_distrs: HashMap::new(),
            delay_modes: DelayModes::default(),
            process_state: None,
            env_state: BasicEnvironmentState::Normal,
            time_to_next_process_event: None,
            time_to_next_delay_event: None,
            scheduled_event: None,
            next_event_index: 0,
            previous_check_time: MonotonicTime::EPOCH,
        }
    }
}

impl CarHoistProcess {
    fn with_distrs(mut self, df: &mut DistributionFactory, configs: HashMap<CarJob, DistributionConfig>) -> Self {
        for (job, config) in configs {
            let distr = df.create(config).expect("Failed to create distribution for job");
            self.job_duration_distrs.insert(job, distr);
        }
        self
    }
}

impl Model for CarHoistProcess {}

impl Process for CarHoistProcess {
    type LogDetailsType = DiscreteProcessLogType<(Worker, Car)>;

    fn pre_update_state(&mut self, source_event_id: &mut EventId, cx: &mut Context<Self>) -> impl Future<Output = ()> {
        async move {
            if let Some((scheduled_time, _)) = self.scheduled_event.as_ref() {
                if *scheduled_time <= cx.time() {
                    self.scheduled_event = None;
                }
            }
        }
    }

    fn update_state_impl(&mut self, source_event_id: &mut EventId, cx: &mut Context<Self>) -> impl Future<Output = ()> {
        async move {
            let time = cx.time();
            let duration_since_prev_check = cx.time().duration_since(self.previous_check_time);

            // Update duration counters based on time since last check
            {
                let is_in_delay = self.delay_modes.active_delay().is_some();
                let is_in_process = self.process_state.is_some() && !is_in_delay;
                let is_env_blocked = matches!(self.env_state, BasicEnvironmentState::Stopped);

                // Decrement process time counter (if not delayed or env blocked)
                if !(is_in_delay || is_env_blocked) {
                    if let (Some((mut process_time_left, (worker, car))), BasicEnvironmentState::Normal) = (self.process_state.take(), &self.env_state) {
                        process_time_left = process_time_left.saturating_sub(duration_since_prev_check);
                        if process_time_left.is_zero() {
                            *source_event_id = self.log(time, source_event_id.clone(), DiscreteProcessLogType::ProcessFinish { resource: (worker.clone(), car.clone()) }).await;
                            self.push_car.send((car, source_event_id.clone())).await;
                            self.push_worker.send((worker, source_event_id.clone())).await;
                        } else {
                            self.process_state = Some((process_time_left, (worker, car)));
                        }
                    }
                }

                // Only case we don't update state here is if no delay is if we don't want the delay counters to decrement,
                // which is only the case if we're not processing and not in a delay - i.e. time-until-delay counters only decrement
                // when a process is active
                if !is_env_blocked && (is_in_delay || is_in_process) {
                    let delay_transition = self.delay_modes.update_state(duration_since_prev_check);
                    if delay_transition.has_changed() {
                        if let Some(delay_name) = &delay_transition.from {
                            *source_event_id = self.log(time, source_event_id.clone(), DiscreteProcessLogType::DelayEnd { delay_name: delay_name.clone() }).await;
                        }
                        if let Some(delay_name) = &delay_transition.to {
                            *source_event_id = self.log(time, source_event_id.clone(), DiscreteProcessLogType::DelayStart { delay_name: delay_name.clone() }).await;
                        }
                    }
                }
            }

            // Update cached environment state
            {
                let new_env_state = match self.req_environment.send(()).await.next() {
                    Some(x) => x,
                    None => BasicEnvironmentState::Normal // Assume always normal operation if no environment state connected
                };
                match (&self.env_state, &new_env_state) {
                    (BasicEnvironmentState::Normal, BasicEnvironmentState::Stopped) => {
                        *source_event_id = self.log(time, source_event_id.clone(), DiscreteProcessLogType::ProcessStopped { reason: "Stopped by environment" }).await;
                        self.env_state = BasicEnvironmentState::Stopped;
                    },
                    (BasicEnvironmentState::Stopped, BasicEnvironmentState::Normal) => {
                        *source_event_id = self.log(time, source_event_id.clone(), DiscreteProcessLogType::ProcessContinue { reason: "Resumed by environment" }).await;
                        self.env_state = BasicEnvironmentState::Normal;
                    }
                    _ => {}
                }
            }

            // Update internal state
            let is_env_stopped = matches!(self.env_state, BasicEnvironmentState::Stopped);
            let has_active_delay = self.delay_modes.active_delay().is_some() || is_env_stopped;
            match (&self.process_state, has_active_delay) {
                (None, false) => {
                    let us_cars_state = self.req_cars_ready.send(()).await.next();
                    let us_workers_state = self.req_workers.send(()).await.next();
                    match (&us_cars_state, &us_workers_state) {
                        (
                            Some(DiscreteStockState::Normal { .. } | DiscreteStockState::Full { .. }),
                            Some(DiscreteStockState::Normal { .. } | DiscreteStockState::Full { .. })
                        ) => {
                            *source_event_id = self.log(time, source_event_id.clone(), DiscreteProcessLogType::WithdrawRequest).await;
                            let received_car = self.withdraw_car.send(((), source_event_id.clone())).await.next().unwrap();
                            let received_worker = self.withdraw_worker.send(((), source_event_id.clone())).await.next().unwrap();
                            match (received_worker, received_car) {
                                (Some(worker), Some(car)) => {
                                    *source_event_id = self.log(time, source_event_id.clone(), DiscreteProcessLogType::ProcessStart { resource: (worker.clone(), car.clone()) }).await;
                                    let mut total_job_duration_secs: f64 = 0.;
                                    for job in car.jobs.iter() {
                                        if let Some(distr) = self.job_duration_distrs.get_mut(job) {
                                            total_job_duration_secs += distr.sample();
                                        } else {
                                            panic!("No job duration distribution defined for job: {:?}", job);
                                        }
                                    }
                                    self.process_state = Some((Duration::from_secs_f64(total_job_duration_secs), (worker, car)));
                                    self.time_to_next_process_event = Some(Duration::from_secs_f64(total_job_duration_secs));
                                },
                                (None, None) => {
                                    *source_event_id = self.log(time, source_event_id.clone(), DiscreteProcessLogType::ProcessNonStart { reason: "No cars or workers ready to service" }).await;
                                    self.time_to_next_process_event = None;
                                },
                                _ => {
                                    panic!("Received only one of car or worker when both (or none) were expected");
                                }
                            }
                        },
                        (Some(DiscreteStockState::Normal { .. } | DiscreteStockState::Full { .. }), Some(DiscreteStockState::Empty { .. })) => {
                            *source_event_id = self.log(time, source_event_id.clone(), DiscreteProcessLogType::ProcessNonStart { reason: "No workers ready to service" }).await;
                            self.time_to_next_process_event = None;
                        },
                        (Some(DiscreteStockState::Normal { .. } | DiscreteStockState::Full { .. }), None) => {
                            *source_event_id = self.log(time, source_event_id.clone(), DiscreteProcessLogType::ProcessNonStart { reason: "No upstream worker pool connected" }).await;
                            self.time_to_next_process_event = None;
                        },
                        (Some(DiscreteStockState::Empty { .. }), Some(DiscreteStockState::Normal { .. } | DiscreteStockState::Full { .. })) => {
                            *source_event_id = self.log(time, source_event_id.clone(), DiscreteProcessLogType::ProcessNonStart { reason: "No cars ready to service" }).await;
                            self.time_to_next_process_event = None;
                        },
                        (None, Some(DiscreteStockState::Normal { .. } | DiscreteStockState::Full { .. })) => {
                            *source_event_id = self.log(time, source_event_id.clone(), DiscreteProcessLogType::ProcessNonStart { reason: "No upstream car queue connected" }).await;
                            self.time_to_next_process_event = None;
                        },
                        _ => {
                            *source_event_id = self.log(time, source_event_id.clone(), DiscreteProcessLogType::ProcessNonStart { reason: "No cars or workers ready to service" }).await;
                            self.time_to_next_process_event = None;
                        }
                    }
                },
                (Some((time, _)), false) => {
                    self.time_to_next_process_event = Some(*time);
                }
                (_, true) => {
                    self.time_to_next_process_event = self.delay_modes.active_delay().map(|(_, delay_state)| *delay_state);
                },
            }
            
            // Set time of next delay
            if self.process_state.is_some() || has_active_delay || !is_env_stopped {
                self.time_to_next_delay_event = self.delay_modes.get_next_event().map(|(_, delay_state)| delay_state.as_duration());
            } else {
                self.time_to_next_delay_event = None;
            }
        }
    }

    fn post_update_state(&mut self, source_event_id: &mut EventId, cx: &mut Context<Self>) -> impl Future<Output = ()> {
        async move {
            match self.time_to_next_process_event {
                None => {},
                Some(time_until_next) => {
                    if time_until_next.is_zero() {
                        panic!("Time until next event is zero!");
                    } else {
                        let next_time = cx.time() + time_until_next;
                        
                        // Schedule event if sooner. If so, cancel previous event.
                        if let Some((scheduled_time, action_key)) = self.scheduled_event.take() {
                            if next_time < scheduled_time {
                                action_key.cancel();
                                let new_event_key =  cx.schedule_keyed_event(next_time, <Self as Process>::update_state, source_event_id.clone()).unwrap();
                                self.scheduled_event = Some((next_time, new_event_key));
                            } else {
                                // Put the event back
                                self.scheduled_event = Some((scheduled_time, action_key));
                            }
                        } else {
                            let new_event_key =  cx.schedule_keyed_event(next_time, <Self as Process>::update_state, source_event_id.clone()).unwrap();
                            self.scheduled_event = Some((next_time, new_event_key));
                        }
                    };
                }
            };
            self.previous_check_time = cx.time();
        }
    }

    fn log(&mut self, now: MonotonicTime, source_event_id: EventId, details: DiscreteProcessLogType<(String, Car)>) -> impl Future<Output = EventId> {
        async move {
            let new_event_id = EventId(format!("{}_{:06}", self.element_code, self.next_event_index));
            let log = DiscreteProcessLog {
                time: now.to_chrono_date_time(0).unwrap().to_string(),
                event_id: new_event_id.clone(),
                source_event_id,
                element_name: self.element_name.clone(),
                element_type: self.element_type.clone(),
                event: details,
            };
            self.next_event_index += 1;
            self.log_emitter.send(log).await;

            new_event_id
        }
    }
}

define_model_enums! {
    pub enum ComponentModel {
        CarSource(DiscreteSource<Car, Car, IncomingCarFactory>, Mailbox<DiscreteSource<Car, Car, IncomingCarFactory>>),
        CarStock(DiscreteStock<Car>, Mailbox<DiscreteStock<Car>>),
        CarHoistProcess(CarHoistProcess, Mailbox<CarHoistProcess>),
        CarSink(DiscreteSink<(), Option<Car>, Car>, Mailbox<DiscreteSink<(), Option<Car>, Car>>),
    }
    pub enum ComponentModelAddress {}
    pub enum ComponentLogger {
        CarStockLogger(DiscreteStockLogger<Car>),
        CarHoistProcessLogger(DiscreteProcessLogger<(Worker, Car)>),
        CarProcessLogger(DiscreteProcessLogger<Car>),
    }
    pub enum ScheduledEventConfig {}
}

impl CustomComponentConnection for ComponentModel {
    fn connect_components(a: &mut Self, b: &mut Self, n: Option<usize>) -> Result<(), Box<dyn Error>> {
        match (a, b) {
            /*
             * Even though we're utilising DiscreteSource etc., we had to add new concrete variants of ComponentModel - so we need to match to them too
             */
            (Self::CarSource(a, am), Self::CarStock(b, bm)) => {
                a.req_downstream.connect(DiscreteStock::get_state_async, bm.address());
                a.push_downstream.connect(DiscreteStock::add, bm.address());
                b.state_emitter.connect(DiscreteSource::update_state, am.address());
                Ok(())
            },
            (Self::CarStock(a, am), Self::CarHoistProcess(b, bm)) => {
                b.req_cars_ready.connect(DiscreteStock::get_state_async, am.address());
                b.withdraw_car.connect(DiscreteStock::remove, am.address());
                a.state_emitter.connect(CarHoistProcess::update_state, bm.address());
                Ok(())
            },
            (Self::CarHoistProcess(a, am), Self::CarStock(b, bm)) => {
                a.push_car.connect(DiscreteStock::add, bm.address());
                // Just going to assume that the car hoist process can always add to the stock - so no need to request state or notify process when there's a change in queue state
                Ok(())
            },
            (Self::StringStock(a, am), Self::CarHoistProcess(b, bm)) => {
                b.req_workers.connect(DiscreteStock::get_state_async, am.address());
                b.withdraw_worker.connect(DiscreteStock::remove, am.address());
                a.state_emitter.connect(CarHoistProcess::update_state, bm.address());
                Ok(())
            },
            (Self::CarHoistProcess(a, am), Self::StringStock(b, bm)) => {
                a.push_worker.connect(DiscreteStock::add, bm.address());
                // Just going to assume that the car hoist process can always add to the stock - so no need to request state or notify process when there's a change in queue state
                Ok(())
            },
            (Self::CarStock(a, am), Self::CarSink(b, bm)) => {
                b.req_upstream.connect(DiscreteStock::get_state_async, am.address());
                b.withdraw_upstream.connect(DiscreteStock::remove, am.address());
                a.state_emitter.connect(DiscreteSink::update_state, bm.address());
                Ok(())
            },
            (a, b) => Err(format!("No component connection defined from {} to {} (n={:?})", a, b, n).into()),
        }
    }
}

impl CustomLoggerConnection for ComponentLogger { 
    type ComponentType = ComponentModel;
    fn connect_logger(a: &mut Self, b: &mut Self::ComponentType, n: Option<usize>) -> Result<(), Box<dyn Error>> {
        match (a, b, n) {
            (ComponentLogger::CarHoistProcessLogger(logger), ComponentModel::CarHoistProcess(process, _),  _) => {
                process.log_emitter.connect_sink(&logger.buffer);
                Ok(())
            },
            (ComponentLogger::CarProcessLogger(logger), ComponentModel::CarSource(process, _),  _) => {
                process.log_emitter.connect_sink(&logger.buffer);
                Ok(())
            },
            (ComponentLogger::CarProcessLogger(logger), ComponentModel::CarSink(process, _),  _) => {
                process.log_emitter.connect_sink(&logger.buffer);
                Ok(())
            },
            (ComponentLogger::CarStockLogger(logger), ComponentModel::CarStock(stock, _),  _) => {
                stock.log_emitter.connect_sink(&logger.buffer);
                Ok(())
            },
            (a, b, _) => Err(format!("No logger connection defined from {} to {} (n={:?})", a, b, n).into()),
        }
    }
}


fn main() {

    let mut df = DistributionFactory::new(12345);

    let mut arrivals = ComponentModel::CarSource(
        DiscreteSource::new()
            .with_name("Arrivals")
            .with_code("A")
            .with_item_factory(IncomingCarFactory::new(&mut df))
            .with_process_time_distr(Distribution::Constant(900.)),
        Mailbox::new()
    );

    let mut ready_to_service = ComponentModel::CarStock(
        DiscreteStock::new()
            .with_name("Ready to Service")
            .with_code("Q1")
            .with_low_capacity(0)
            .with_max_capacity(3),
        Mailbox::new()
    );

    let mut car_hoists: Vec<ComponentModel> = (0..1).into_iter().map(|i| {
        ComponentModel::CarHoistProcess(
            CarHoistProcess::new()
                .with_name(&format!("Car Hoist {}", i))
                .with_code(&format!("P{}", i))
                .with_distrs(
                    &mut df,
                    HashMap::from([
                        (CarJob::ReplaceTyres, DistributionConfig::Constant(600.)),
                        (CarJob::ChangeOil, DistributionConfig::Constant(1200.)),
                        (CarJob::ReplaceBrakes, DistributionConfig::Constant(900.)),
                    ])
                ),
            Mailbox::new()
        )
    }).collect::<Vec<_>>();

    let mut ready_to_depart = ComponentModel::CarStock(
        DiscreteStock::new()
            .with_name("Ready to Depart")
            .with_code("Q2")
            .with_low_capacity(0)
            .with_max_capacity(3),
        Mailbox::new()
    );

    let mut departures = ComponentModel::CarSink(
        DiscreteSink::new()
            .with_name("Departures")
            .with_code("D")
            .with_process_time_distr(Distribution::Constant(1.)),
        Mailbox::new()
    );

    let mut worker_pool = ComponentModel::StringStock(
        DiscreteStock::new()
            .with_name("Worker Pool")
            .with_code("WP")
            .with_low_capacity(0)
            .with_max_capacity(99)
            .with_initial_resource(ItemDeque::from(vec!["Albert".into(), "Becky".into(), "Charlie".into()])),
        Mailbox::new()
    );

    connect_components!(&mut arrivals, &mut ready_to_service).unwrap();

    for mut hoist in car_hoists.iter_mut() {
        connect_components!(&mut ready_to_service, &mut hoist).unwrap();
        connect_components!(&mut hoist, &mut ready_to_depart).unwrap();

        connect_components!(&mut worker_pool, &mut hoist).unwrap();
        connect_components!(&mut hoist, &mut worker_pool).unwrap();
    }
    connect_components!(&mut ready_to_depart, &mut departures).unwrap();

    let mut hoist_process_logger = ComponentLogger::CarHoistProcessLogger(DiscreteProcessLogger::new("HoistProcessLogger"));
    let mut car_process_logger = ComponentLogger::CarProcessLogger(DiscreteProcessLogger::new("ProcessLogger"));
    let mut car_stock_logger = ComponentLogger::CarStockLogger(DiscreteStockLogger::new("StockLogger"));
    let mut worker_pool_logger = ComponentLogger::StringStockLogger(DiscreteStockLogger::new("WorkerPoolLogger"));

    connect_logger!(&mut car_process_logger, &mut arrivals).unwrap();
    for mut car_hoist in car_hoists.iter_mut() {
        connect_logger!(&mut hoist_process_logger, &mut car_hoist).unwrap();
    }
    connect_logger!(&mut car_process_logger, &mut departures).unwrap();
    connect_logger!(&mut car_stock_logger, &mut ready_to_service).unwrap();
    connect_logger!(&mut car_stock_logger, &mut ready_to_depart).unwrap();

    connect_logger!(&mut worker_pool_logger, &mut worker_pool).unwrap();

    let mut sim_init = SimInit::new();
    sim_init = register_component!(sim_init, arrivals);
    sim_init = register_component!(sim_init, ready_to_service);
    for hoist in car_hoists {
        sim_init = register_component!(sim_init, hoist);
    }
    sim_init = register_component!(sim_init, ready_to_depart);
    sim_init = register_component!(sim_init, departures);
    
    sim_init = register_component!(sim_init, worker_pool);

    let start_time = MonotonicTime::try_from_date_time(2025, 7, 1, 8, 0, 0, 0).unwrap();
    let (mut sim, mut scheduler) = sim_init.init(start_time).unwrap();
    sim.step_until(start_time + Duration::from_secs(3600 * 9)).unwrap();

    let output_dir = "outputs/car_workshop_2";
    create_dir_all(output_dir).unwrap();
    hoist_process_logger.write_csv(output_dir).unwrap();
    car_process_logger.write_csv(output_dir).unwrap();
    car_stock_logger.write_csv(output_dir).unwrap();
}