use quokkasim::prelude::*;
use core::f64;
use std::collections::HashMap;
use std::time::{Duration};
use std::fmt::Debug;
use serde::{Serialize};    

#[derive(Clone, Debug, Serialize)]
struct Reactants {
    pub oil: f64,
    pub methanol: f64,
    pub biodiesel: f64,
    pub glycerin: f64,
    pub water: f64,
}

impl ContArithmetic for Reactants {
    fn add(&mut self, arg: Self) {
        self.oil += arg.oil;
        self.methanol += arg.methanol;
        self.biodiesel += arg.biodiesel;
        self.glycerin += arg.glycerin;
        self.water += arg.water;
    }

    fn multiply(&mut self, arg: f64) {
        self.oil *= arg;
        self.methanol *= arg;
        self.biodiesel *= arg;
        self.glycerin *= arg;
        self.water *= arg;
    }

    fn remove<T>(&mut self, arg: T) -> Self where Self: Projectable<T> {
        let removed = self.clone().project(arg);
        self.oil -= removed.oil;
        self.methanol -= removed.methanol;
        self.biodiesel -= removed.biodiesel;
        self.glycerin -= removed.glycerin;
        self.water -= removed.water;
        removed
    }

    fn remove_all(&mut self) -> Self {
        let removed = self.clone();
        self.multiply(0.);
        removed
    }

    fn total(&self) -> f64 {
        self.oil + self.methanol + self.biodiesel + self.glycerin + self.water
    }
}

impl Projectable<f64> for Reactants {
    fn project(self, x: f64) -> Self {
        let total = self.oil + self.methanol + self.biodiesel + self.glycerin + self.water;
        let proportion = if total > 0.0 { x / total } else { 0.0 };
        Reactants {
            oil: self.oil * proportion,
            methanol: self.methanol * proportion,
            biodiesel: self.biodiesel * proportion,
            glycerin: self.glycerin * proportion,
            water: self.water * proportion,
        }
    }
}

impl Projectable<Reactants> for Reactants {
    fn project(self, x: Reactants) -> Self {
        x
    }
}

impl Default for Reactants {
    fn default() -> Self {
        Reactants {
            oil: 0.0,
            methanol: 0.0,
            biodiesel: 0.0,
            glycerin: 0.0,
            water: 0.0,
        }
    }
}

#[derive(WithMethods)]
struct ReactionVessel {
    // Identification
    pub element_name: String,
    pub element_code: String,
    pub element_type: String,

    // Ports
    pub req_upstream_oil: Requestor<(), ContStockState>,
    pub req_upstream_methanol: Requestor<(), ContStockState>,
    pub req_downstream: Requestor<(), ContStockState>,
    pub withdraw_upstream_oil: Requestor<(f64, EventId), Reactants>,
    pub withdraw_upstream_methanol: Requestor<(f64, EventId), Reactants>,
    pub push_downstream: Output<(Reactants, EventId)>,
    pub log_emitter: Output<ContProcessLog<Reactants>>,

    // Configuration
    pub process_quantity_distr: Distribution,
    pub process_time_distr: Distribution,

    // Runtime State
    pub process_state: Option<(Duration, Reactants)>,

    // Internals
    pub time_to_next_process_event: Option<Duration>,
    pub scheduled_event: Option<(MonotonicTime, ActionKey)>,
    pub next_event_index: u64,
    pub previous_check_time: MonotonicTime,
}

impl Default for ReactionVessel {
    fn default() -> Self {
        ReactionVessel {
            element_name: "ReactionVessel".to_string(),
            element_code: "RV".to_string(),
            element_type: "DefaultContProcess".to_string(),
            req_upstream_oil: Requestor::new(),
            req_upstream_methanol: Requestor::new(),
            req_downstream: Requestor::new(),
            withdraw_upstream_oil: Requestor::new(),
            withdraw_upstream_methanol: Requestor::new(),
            push_downstream: Output::new(),
            log_emitter: Output::new(),
            process_quantity_distr: Distribution::Constant(1.0),
            process_time_distr: Distribution::Constant(60.0),
            process_state: None,
            time_to_next_process_event: None,
            scheduled_event: None,
            next_event_index: 0,
            previous_check_time: MonotonicTime::EPOCH,
        }
    }
}

impl Model for ReactionVessel {
    fn init(
        mut self,
        ctx: &mut Context<Self>,
    ) -> impl Future<Output = InitializedModel<Self>> + Send {
        async move {
            let source_event_id = EventId(format!(
                "{}_{:06}",
                self.element_code, self.next_event_index
            ));
            self.update_state(source_event_id, ctx).await;
            println!("Initialized ReactionVessel");
            self.into()
        }
    }
}

impl ContProcessCore<Reactants, ContProcessLog<Reactants>> for ReactionVessel {
    fn element_name(&self) -> &str { &self.element_name }
    fn element_code(&self) -> &str { &self.element_code }
    fn element_type(&self) -> &str { &self.element_type }
    fn get_next_event_id(&mut self) -> EventId {
        let event_id = EventId(format!(
            "{}_{:06}",
            self.element_code, self.next_event_index
        ));
        self.next_event_index += 1;
        event_id
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
    fn update_state(
            &mut self,
            mut source_event_id: EventId,
            cx: &mut Context<Self>,
        ) -> impl Future<Output = ()> + Send {
        async move {
            self.update_state_since_last_update(&mut source_event_id, cx).await;
            self.update_state_decision_logic(&mut source_event_id, cx).await;
            self.update_state_for_next_event(&mut source_event_id, cx).await;
        }
    }
}

impl ContProcessUpdateSinceLast<Reactants, ContProcessLog<Reactants>> for ReactionVessel {
    fn update_process_state_since_prev_event(
        &mut self, source_event_id: &mut EventId,
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
                    self.time_to_next_process_event = None;
                } else {
                    self.process_state = Some((time_left, resource));
                    self.time_to_next_process_event = Some(time_left);
                }
            }
        }
    }

    fn log_type_process_success(&mut self, source_event_id: &mut EventId, quantity: f64, resource: Reactants, cx: &mut Context<Self>) -> impl Future<Output = EventId> {
        async move {
            let current_event_id = self.get_next_event_id();
            self.log_emitter.send(ContProcessLog {
                time: cx.time().to_chrono_date_time(0).unwrap().to_string(),
                event_id: current_event_id.clone(),
                source_event_id: source_event_id.clone(),
                element_name: self.element_name.clone(),
                element_type: self.element_type.clone(),
                details: DefaultContProcessLogType::ProcessSuccess { quantity, resource },
            }).await;
            current_event_id
        }
    }
}

impl ContProcessUpdateDecisionLogic<Reactants, ContProcessLog<Reactants>> for ReactionVessel {
    fn update_state_decision_logic(
            &mut self,
            source_event_id: &mut EventId,
            cx: &mut Context<Self>,
        ) -> impl Future<Output = ()> {
        async move {
            match &self.process_state {
                None => {
                    let us_oil_state = self.req_upstream_oil.send(()).await.next();
                    let us_methanol_state = self.req_upstream_methanol.send(()).await.next();
                    let ds_state = self.req_downstream.send(()).await.next();

                    match (&us_oil_state, &us_methanol_state, &ds_state) {
                        (
                            Some(ContStockState::Normal { .. })
                            | Some(ContStockState::Full { .. }),
                            Some(ContStockState::Normal { .. })
                            | Some(ContStockState::Full { .. }),
                            Some(ContStockState::Empty { .. })
                            | Some(ContStockState::Normal { .. }),
                        ) => {
                            let process_quantity = self.process_quantity_distr.sample();
                            *source_event_id = self.log_type_withdraw_request(source_event_id, process_quantity, cx).await;
                            let mut moved_oil: Reactants = self.withdraw_upstream_oil.send((process_quantity, source_event_id.clone())).await.next().unwrap();
                            let moved_methanol: Reactants = self.withdraw_upstream_methanol.send((process_quantity, source_event_id.clone())).await.next().unwrap();
                            moved_oil.add(moved_methanol);

                            let process_duration_secs = self.process_time_distr.sample();
                            self.process_state = Some((
                                Duration::from_secs_f64(process_duration_secs),
                                moved_oil.clone()
                            ));
                            *source_event_id = self.log_type_process_start(source_event_id, process_quantity, moved_oil.clone(), cx).await;
                            self.time_to_next_process_event = Some(Duration::from_secs_f64(process_duration_secs));
                        }
                        (Some(ContStockState::Empty { .. }), _, _) | (_, Some(ContStockState::Empty { .. }), _) => {
                            *source_event_id = self.log_type_process_failure(source_event_id, "Upstream is empty", cx).await;
                            self.time_to_next_process_event = None;
                        }
                        (None, _, _) | (_, None, _) => {
                            *source_event_id = self.log_type_process_failure(source_event_id, "Upstream is not connected", cx).await;
                            self.time_to_next_process_event = None;
                        }
                        (_, _, None) => {
                            *source_event_id = self.log_type_process_failure(source_event_id, "Downstream is not connected", cx).await;
                            self.time_to_next_process_event = None;
                        }
                        (_, _, Some(ContStockState::Full { .. })) => {
                            *source_event_id = self.log_type_process_failure(source_event_id, "Downstream is full", cx).await;
                            self.time_to_next_process_event = None;
                        }
                    }
                }
                Some((time, _)) => {
                    self.time_to_next_process_event = Some(*time);
                }
            }
        }
    }
    
    fn log_type_withdraw_request(&mut self, source_event_id: &mut EventId, quantity: f64, cx: &mut Context<Self>) -> impl Future<Output = EventId> {
        async move {
            let current_event_id = self.get_next_event_id();
            self.log_emitter.send(ContProcessLog {
                time: cx.time().to_chrono_date_time(0).unwrap().to_string(),
                event_id: current_event_id.clone(),
                source_event_id: source_event_id.clone(),
                element_name: self.element_name.clone(),
                element_type: self.element_type.clone(),
                details: DefaultContProcessLogType::WithdrawRequest { quantity },
            }).await;
            current_event_id
        }
    }

    fn log_type_process_start(&mut self, source_event_id: &mut EventId, quantity: f64, resource: Reactants, cx: &mut Context<Self>) -> impl Future<Output = EventId> {
        async move {
            let current_event_id = self.get_next_event_id();
            self.log_emitter.send(ContProcessLog {
                time: cx.time().to_chrono_date_time(0).unwrap().to_string(),
                event_id: current_event_id.clone(),
                source_event_id: source_event_id.clone(),
                element_name: self.element_name.clone(),
                element_type: self.element_type.clone(),
                details: DefaultContProcessLogType::ProcessStart { quantity, resource },
            }).await;
            current_event_id
        }
    }

    fn log_type_process_failure(&mut self, source_event_id: &mut EventId, reason: &'static str, cx: &mut Context<Self>) -> impl Future<Output = EventId> {
        async move {
            let current_event_id = self.get_next_event_id();
            self.log_emitter.send(ContProcessLog {
                time: cx.time().to_chrono_date_time(0).unwrap().to_string(),
                event_id: current_event_id.clone(),
                source_event_id: source_event_id.clone(),
                element_name: self.element_name.clone(),
                element_type: self.element_type.clone(),
                details: DefaultContProcessLogType::ProcessFailure { reason },
            }).await;
            current_event_id
        }
    }
}

impl ContProcessUpdateForNextEvent<Reactants, ContProcessLog<Reactants>> for ReactionVessel {}

impl Connect<DefaultContStock<Reactants, ContStockState, ContStockLog<Reactants>>, ReactionVessel> for Connection {
    fn connect(
        &mut self,
        a: (
            &mut DefaultContStock<Reactants, ContStockState, ContStockLog<Reactants>>,
            &Address<DefaultContStock<Reactants, ContStockState, ContStockLog<Reactants>>>,
            Option<usize>
        ),
        b: (
            &mut ReactionVessel,
            &Address<ReactionVessel>,
            Option<usize>
    )) -> Result<(), String> {
        match b.2 {
            Some(0) => {
                // Connect to oil input port
                b.0.req_upstream_oil.connect(DefaultContStock::get_state_async, a.1.clone());
                b.0.withdraw_upstream_oil.connect(DefaultContStock::remove, a.1.clone());
            }
            Some(1) => {
                // Connect to methanol input port
                b.0.req_upstream_methanol.connect(DefaultContStock::get_state_async, a.1.clone());
                b.0.withdraw_upstream_methanol.connect(DefaultContStock::remove, a.1.clone());
            },
            Some(_) => {
                return Err("Invalid port index when connecting DefaultContStock to ReactionVessel".to_string());
            }
            None => {
                return Err("Must specify port index when connecting DefaultContStock to ReactionVessel".to_string());
            }
        };
        a.0.state_emitter.connect(ReactionVessel::update_state, b.1.clone());
        Ok(())
    }
}


impl Connect<ReactionVessel, DefaultContStock<Reactants, ContStockState, ContStockLog<Reactants>>> for Connection {
    fn connect(
        &mut self,
        a: (
            &mut ReactionVessel,
            &Address<ReactionVessel>,
            Option<usize>
        ),
        b: (
            &mut DefaultContStock<Reactants, ContStockState, ContStockLog<Reactants>>,
            &Address<DefaultContStock<Reactants, ContStockState, ContStockLog<Reactants>>>,
            Option<usize>
        )) -> Result<(), String> {
        a.0.req_downstream.connect(DefaultContStock::get_state_async, b.1.clone());
        a.0.push_downstream.connect(DefaultContStock::add, b.1.clone());
        b.0.state_emitter.connect(ReactionVessel::update_state, a.1.clone());
        Ok(())
    }
}

#[derive(WithMethods)]
struct Splitter {
    // Identification
    pub element_name: String,
    pub element_code: String,
    pub element_type: String,

    // Ports
    pub req_upstream: Requestor<(), ContStockState>,
    pub req_downstream_1: Requestor<(), ContStockState>,
    pub req_downstream_2: Requestor<(), ContStockState>,
    pub withdraw_upstream: Requestor<(f64, EventId), Reactants>,
    pub push_downstream_1: Output<(Reactants, EventId)>,
    pub push_downstream_2: Output<(Reactants, EventId)>,
    pub log_emitter: Output<ContProcessLog<Reactants>>,

    // Configuration
    pub process_quantity_distr: Distribution,
    pub process_time_distr: Distribution,
    pub splitter_fn: Box<dyn FnMut(&mut HashMap<String, Distribution>, &Reactants) -> (Reactants, Reactants) + Send + Sync>,
    pub parameter_distrs: HashMap<String, Distribution>,

    // Runtime State
    pub process_state: Option<(Duration, Reactants)>,

    // Internals
    pub time_to_next_process_event: Option<Duration>,
    pub scheduled_event: Option<(MonotonicTime, ActionKey)>,
    pub next_event_index: u64,
    pub previous_check_time: MonotonicTime,
}

impl Default for Splitter {
    fn default() -> Self {
        Splitter {
            element_name: "Splitter".to_string(),
            element_code: "SP".to_string(),
            element_type: "DefaultContProcess".to_string(),
            req_upstream: Requestor::new(),
            req_downstream_1: Requestor::new(),
            req_downstream_2: Requestor::new(),
            withdraw_upstream: Requestor::new(),
            push_downstream_1: Output::new(),
            push_downstream_2: Output::new(),
            log_emitter: Output::new(),
            process_quantity_distr: Distribution::Constant(1.0),
            process_time_distr: Distribution::Constant(60.0),
            splitter_fn: Box::new(|_: &mut HashMap<String, Distribution>, resource: &Reactants| {
                let half_resource = {
                    let mut r = resource.clone();
                    r.multiply(0.5);
                    r
                };
                (half_resource.clone(), half_resource)
            }),
            parameter_distrs: HashMap::new(),
            process_state: None,
            time_to_next_process_event: None,
            scheduled_event: None,
            next_event_index: 0,
            previous_check_time: MonotonicTime::EPOCH,
        }
    }
}

impl Model for Splitter {
    fn init(
        mut self,
        ctx: &mut Context<Self>,
    ) -> impl Future<Output = InitializedModel<Self>> + Send {
        async move {
            let source_event_id = EventId(format!(
                "{}_{:06}",
                self.element_code, self.next_event_index
            ));
            self.update_state(source_event_id, ctx).await;
            println!("Initialized Splitter {}", self.element_name);
            self.into()
        }
    }
}

impl ContProcessCore<Reactants, ContProcessLog<Reactants>> for Splitter {
    fn element_name(&self) -> &str { &self.element_name }
    fn element_code(&self) -> &str { &self.element_code }
    fn element_type(&self) -> &str { &self.element_type }
    fn get_next_event_id(&mut self) -> EventId {
        let event_id = EventId(format!(
            "{}_{:06}",
            self.element_code, self.next_event_index
        ));
        self.next_event_index += 1;
        event_id
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
    fn update_state(
            &mut self,
            mut source_event_id: EventId,
            cx: &mut Context<Self>,
        ) -> impl Future<Output = ()> + Send {
        async move {
            // Implementation of state update logic goes here
            self.update_state_since_last_update(&mut source_event_id, cx).await;
            self.update_state_decision_logic(&mut source_event_id, cx).await;
            self.update_state_for_next_event(&mut source_event_id, cx).await;
        }
    }
}

impl ContProcessUpdateSinceLast<Reactants, ContProcessLog<Reactants>> for Splitter {
    fn update_process_state_since_prev_event(
        &mut self, source_event_id: &mut EventId,
        cx: &mut Context<Self>,
        duration_since_prev: Duration
    ) -> impl Future<Output = ()> {
        async move {
            if let Some((mut time_left, resource)) = self.process_state.take() {
                time_left = time_left.saturating_sub(duration_since_prev);
                if time_left.is_zero() {
                    let (output_1, output_2) = (self.splitter_fn)(&mut self.parameter_distrs, &resource);
                    let log_payload_1 = output_1.clone();
                    let log_payload_2 = output_2.clone();
                    let event_id_1 = self.log_type_process_success(source_event_id, output_1.total(), log_payload_1, cx).await;
                    let event_id_2 = self.log_type_process_success(source_event_id, output_2.total(), log_payload_2, cx).await;
                    self.push_downstream_1
                        .send((output_1.clone(), event_id_1))
                        .await;
                    self.push_downstream_2
                        .send((output_2.clone(), event_id_2))
                        .await;
                    self.time_to_next_process_event = None;
                } else {
                    self.process_state = Some((time_left, resource));
                    self.time_to_next_process_event = Some(time_left);
                }
            }
        }
    }

    fn log_type_process_success(&mut self, source_event_id: &mut EventId, quantity: f64, resource: Reactants, cx: &mut Context<Self>) -> impl Future<Output = EventId> {
        async move {
            let current_event_id = self.get_next_event_id();
            self.log_emitter.send(ContProcessLog {
                time: cx.time().to_chrono_date_time(0).unwrap().to_string(),
                event_id: current_event_id.clone(),
                source_event_id: source_event_id.clone(),
                element_name: self.element_name.clone(),
                element_type: self.element_type.clone(),
                details: DefaultContProcessLogType::ProcessSuccess { quantity, resource },
            }).await;
            current_event_id
        }
    }
}

impl ContProcessUpdateDecisionLogic<Reactants, ContProcessLog<Reactants>> for Splitter {
fn update_state_decision_logic(
            &mut self,
            source_event_id: &mut EventId,
            cx: &mut Context<Self>,
        ) -> impl Future<Output = ()> {
        async move {
            match &self.process_state {
                None => {
                    let ds_state_1 = self.req_downstream_1.send(()).await.next();
                    let ds_state_2 = self.req_downstream_2.send(()).await.next();
                    let us_state = self.req_upstream.send(()).await.next();

                    match (&us_state, &ds_state_1, &ds_state_2) {
                        (
                            Some(ContStockState::Normal { .. })
                            | Some(ContStockState::Full { .. }),
                            Some(ContStockState::Empty { .. })
                            | Some(ContStockState::Normal { .. }),
                            Some(ContStockState::Empty { .. })
                            | Some(ContStockState::Normal { .. }),
                        ) => {
                            let process_quantity = self.process_quantity_distr.sample();
                            *source_event_id = self.log_type_withdraw_request(source_event_id, process_quantity, cx).await;
                            let resource_to_process = self.withdraw_upstream.send((process_quantity, source_event_id.clone())).await.next().unwrap();

                            let process_duration_secs = self.process_time_distr.sample();
                            self.process_state = Some((
                                Duration::from_secs_f64(process_duration_secs),
                                resource_to_process.clone()
                            ));
                            *source_event_id = self.log_type_process_start(source_event_id, process_quantity, resource_to_process.clone(), cx).await;
                            self.time_to_next_process_event = Some(Duration::from_secs_f64(process_duration_secs));
                        }
                        (Some(ContStockState::Empty { .. }), _, _) | (_, Some(ContStockState::Empty { .. }), _) => {
                            *source_event_id = self.log_type_process_failure(source_event_id, "Upstream is empty", cx).await;
                            self.time_to_next_process_event = None;
                        }
                        (None, _, _) => {
                            *source_event_id = self.log_type_process_failure(source_event_id, "Upstream is not connected", cx).await;
                            self.time_to_next_process_event = None;
                        }
                        (_, _, None) | (_, None, _) => {
                            *source_event_id = self.log_type_process_failure(source_event_id, "Downstream is not connected", cx).await;
                            self.time_to_next_process_event = None;
                        }
                        (_, _, Some(ContStockState::Full { .. })) | (_, Some(ContStockState::Full { .. }), _) => {
                            *source_event_id = self.log_type_process_failure(source_event_id, "Downstream is full", cx).await;
                            self.time_to_next_process_event = None;
                        }
                    }
                }
                Some((time, _)) => {
                    self.time_to_next_process_event = Some(*time);
                }
            }
        }
    }

    fn log_type_withdraw_request(&mut self, source_event_id: &mut EventId, quantity: f64, cx: &mut Context<Self>) -> impl Future<Output = EventId> {
        async move {
            let current_event_id = self.get_next_event_id();
            self.log_emitter.send(ContProcessLog {
                time: cx.time().to_chrono_date_time(0).unwrap().to_string(),
                event_id: current_event_id.clone(),
                source_event_id: source_event_id.clone(),
                element_name: self.element_name.clone(),
                element_type: self.element_type.clone(),
                details: DefaultContProcessLogType::WithdrawRequest { quantity },
            }).await;
            current_event_id
        }
    }

    fn log_type_process_start(&mut self, source_event_id: &mut EventId, quantity: f64, resource: Reactants, cx: &mut Context<Self>) -> impl Future<Output = EventId> {
        async move {
            let current_event_id = self.get_next_event_id();
            self.log_emitter.send(ContProcessLog {
                time: cx.time().to_chrono_date_time(0).unwrap().to_string(),
                event_id: current_event_id.clone(),
                source_event_id: source_event_id.clone(),
                element_name: self.element_name.clone(),
                element_type: self.element_type.clone(),
                details: DefaultContProcessLogType::ProcessStart { quantity, resource },
            }).await;
            current_event_id
        }
    }

    fn log_type_process_failure(&mut self, source_event_id: &mut EventId, reason: &'static str, cx: &mut Context<Self>) -> impl Future<Output = EventId> {
        async move {
            let current_event_id = self.get_next_event_id();
            self.log_emitter.send(ContProcessLog {
                time: cx.time().to_chrono_date_time(0).unwrap().to_string(),
                event_id: current_event_id.clone(),
                source_event_id: source_event_id.clone(),
                element_name: self.element_name.clone(),
                element_type: self.element_type.clone(),
                details: DefaultContProcessLogType::ProcessFailure { reason },
            }).await;
            current_event_id
        }
    }
}

impl ContProcessUpdateForNextEvent<Reactants, ContProcessLog<Reactants>> for Splitter {}

impl Splitter {
    pub fn with_splitter_fn<F>(mut self, splitter_fn: F) -> Self
    where
        F: FnMut(&mut HashMap<String, Distribution>, &Reactants) -> (Reactants, Reactants) + Send + Sync + 'static,
    {
        self.splitter_fn = Box::new(splitter_fn);
        self
    }
}

impl Connect<DefaultContStock<Reactants, ContStockState, ContStockLog<Reactants>>, Splitter> for Connection {
    fn connect(
        &mut self,
        a: (
            &mut DefaultContStock<Reactants, ContStockState, ContStockLog<Reactants>>,
            &Address<DefaultContStock<Reactants, ContStockState, ContStockLog<Reactants>>>,
            Option<usize>
        ),
        b: (
            &mut Splitter,
            &Address<Splitter>,
            Option<usize>
    )) -> Result<(), String> {
        b.0.req_upstream.connect(DefaultContStock::get_state_async, a.1.clone());
        b.0.withdraw_upstream.connect(DefaultContStock::remove, a.1.clone());
        a.0.state_emitter.connect(Splitter::update_state, b.1.clone());
        Ok(())
    }
}

impl Connect<Splitter, DefaultContStock<Reactants, ContStockState, ContStockLog<Reactants>>> for Connection {
    fn connect(
        &mut self,
        a: (
            &mut Splitter,
            &Address<Splitter>,
            Option<usize>
        ),
        b: (
            &mut DefaultContStock<Reactants, ContStockState, ContStockLog<Reactants>>,
            &Address<DefaultContStock<Reactants, ContStockState, ContStockLog<Reactants>>>,
            Option<usize>
        )) -> Result<(), String> {
        match a.2 {
            Some(0) => {
                // Connect to downstream 1 port
                a.0.req_downstream_1.connect(DefaultContStock::get_state_async, b.1.clone());
                a.0.push_downstream_1.connect(DefaultContStock::add, b.1.clone());
            }
            Some(1) => {
                // Connect to downstream 2 port
                a.0.req_downstream_2.connect(DefaultContStock::get_state_async, b.1.clone());
                a.0.push_downstream_2.connect(DefaultContStock::add, b.1.clone());
            },
            Some(_) => {
                return Err("Invalid port index when connecting Splitter to DefaultContStock".to_string());
            }
            None => {
                return Err("Must specify port index when connecting Splitter to DefaultContStock".to_string());
            }
        };
        b.0.state_emitter.connect(Splitter::update_state, a.1.clone());
        Ok(())
    }
}

fn main() {
    let mut df = DistributionFactory::new(97531);

    let mut oil_tank: DefaultContStock<Reactants, ContStockState, ContStockLog<Reactants>> = DefaultContStock::new()
        .with_name("OilStock")
        .with_code("OS")
        .with_initial_resource(Reactants { oil: 1000.0, methanol: 0.0, biodiesel: 0.0, glycerin: 0.0, water: 0.0 })
        .with_max_capacity(f64::INFINITY);
    let (oil_tank_mbox, oil_tank_addr) = oil_tank.create_mailbox();

    let mut methanol_tank: DefaultContStock<Reactants, ContStockState, ContStockLog<Reactants>> = DefaultContStock::new()
        .with_name("MethanolTank")
        .with_code("MT")
        .with_initial_resource(Reactants { oil: 0.0, methanol: 500.0, biodiesel: 0.0, glycerin: 0.0, water: 0.0 })
        .with_max_capacity(f64::INFINITY);
    let (methanol_tank_mbox, methanol_tank_addr) = methanol_tank.create_mailbox();

    let mut crude_glycerin_tank: DefaultContStock<Reactants, ContStockState, ContStockLog<Reactants>> = DefaultContStock::new()
        .with_name("CrudeGlycerinTank")
        .with_code("CGT")
        .with_initial_resource(Reactants { oil: 0.0, methanol: 0.0, biodiesel: 0.0, glycerin: 200.0, water: 0.0 })
        .with_max_capacity(10_000.);
    let (crude_glycerin_tank_mbox, crude_glycerin_tank_addr) = crude_glycerin_tank.create_mailbox();

    let mut reaction_vessel = ReactionVessel::default()
        .with_name("ReactionVessel")
        .with_code("RV")
        .with_process_time_distr(df.create(DistributionConfig::TruncNormal { mean: 180.0, std: 15.0, min: Some(1.), max: None }).unwrap())
        .with_process_quantity_distr(df.create(DistributionConfig::TruncNormal { mean: 100.0, std: 10.0, min: Some(1.), max: None }).unwrap());
    let (reaction_vessel_mbox, reaction_vessel_addr) = reaction_vessel.create_mailbox();

    let mut reaction_to_decant_stock: DefaultContStock<Reactants, ContStockState, ContStockLog<Reactants>> = DefaultContStock::new()
        .with_name("ReactionToDecantStock")
        .with_code("RDS")
        .with_max_capacity(1.0);
    let (reaction_to_decant_stock_mbox, reaction_to_decant_stock_addr) = reaction_to_decant_stock.create_mailbox();

    let mut decantation_unit = Splitter::default()
        .with_name("DecantationUnit")
        .with_code("DECU")
        .with_process_time_distr(df.create(DistributionConfig::TruncNormal { mean: 120.0, std: 10.0, min: Some(1.), max: None }).unwrap())
        .with_process_quantity_distr(df.create(DistributionConfig::TruncNormal { mean: 100.0, std: 10.0, min: Some(1.), max: None }).unwrap())
        .with_splitter_fn(Box::new(|distr_map: &mut HashMap<String, Distribution>, resource: &Reactants| {
            let glycerin_recovery_rate = distr_map.get_mut("glycerin_recovery_rate").map_or(0.9, |d| d.sample()).clamp(0., 1.);
            let methanol_recovery_rate = distr_map.get_mut("methanol_recovery_rate").map_or(0.3, |d| d.sample()).clamp(0., 1.);
            let glycerin_stream = Reactants {
                oil: 0.0,
                methanol: resource.methanol * methanol_recovery_rate,
                biodiesel: 0.0,
                glycerin: resource.glycerin * glycerin_recovery_rate,
                water: 0.0,
            };
            let biodiesel_stream = resource.clone().remove::<Reactants>(glycerin_stream.clone());
            (biodiesel_stream, glycerin_stream)
        }));
    let (decantation_unit_mbox, decantation_unit_addr) = decantation_unit.create_mailbox();

    let mut decant_to_drying_stock: DefaultContStock<Reactants, ContStockState, ContStockLog<Reactants>> = DefaultContStock::new()
        .with_name("DecantToDryingStock")
        .with_code("DDS")
        .with_max_capacity(1.0);

    let (decant_to_drying_stock_mbox, decant_to_drying_stock_addr) = decant_to_drying_stock.create_mailbox();

    let mut drying_unit: Splitter = Splitter::default()
        .with_name("DryingUnit")
        .with_code("DRYU")
        .with_process_time_distr(df.create(DistributionConfig::TruncNormal { mean: 60.0, std: 5.0, min: Some(1.), max: None }).unwrap())
        .with_process_quantity_distr(df.create(DistributionConfig::TruncNormal { mean: 100.0, std: 10.0, min: Some(1.), max: None }).unwrap())
        .with_splitter_fn(Box::new(|distr_map: &mut HashMap<String, Distribution>, resource: &Reactants| {
            let methanol_recovery_rate = distr_map.get_mut("methanol_recovery_rate").map_or(0.95, |d| d.sample()).clamp(0., 1.);
            let methanol_stream = Reactants {
                oil: 0.0,
                methanol: resource.methanol * methanol_recovery_rate,
                biodiesel: 0.0,
                glycerin: 0.0,
                water: 0.0,
            };
            let biodiesel_stream = resource.clone().remove::<Reactants>(methanol_stream.clone());
            (biodiesel_stream, methanol_stream)
        }));
    let (drying_unit_mbox, drying_unit_addr) = drying_unit.create_mailbox();

    let mut product_stock: DefaultContStock<Reactants, ContStockState, ContStockLog<Reactants>> = DefaultContStock::new()
        .with_name("ProductStock")
        .with_code("PS")
        .with_max_capacity(f64::INFINITY);
    let (product_stock_mbox, product_stock_addr) = product_stock.create_mailbox();

    // Connections

    let mut c = Connection {};
    c.connect((&mut oil_tank, &oil_tank_addr, None), (&mut reaction_vessel, &reaction_vessel_addr, Some(0))).unwrap();
    c.connect((&mut methanol_tank, &methanol_tank_addr, None), (&mut reaction_vessel, &reaction_vessel_addr, Some(1))).unwrap();
    c.connect((&mut reaction_vessel, &reaction_vessel_addr, None), (&mut reaction_to_decant_stock, &reaction_to_decant_stock_addr, None)).unwrap();
    c.connect((&mut reaction_to_decant_stock, &reaction_to_decant_stock_addr, None), (&mut decantation_unit, &decantation_unit_addr, None)).unwrap();
    c.connect((&mut decantation_unit, &decantation_unit_addr, Some(0)), (&mut decant_to_drying_stock, &decant_to_drying_stock_addr, None)).unwrap();
    c.connect((&mut decantation_unit, &decantation_unit_addr, Some(1)), (&mut crude_glycerin_tank, &crude_glycerin_tank_addr, None)).unwrap();
    c.connect((&mut decant_to_drying_stock, &decant_to_drying_stock_addr, None), (&mut drying_unit, &drying_unit_addr, None)).unwrap();
    c.connect((&mut drying_unit, &drying_unit_addr, Some(0)), (&mut product_stock, &product_stock_addr, None)).unwrap();
    c.connect((&mut drying_unit, &drying_unit_addr, Some(1)), (&mut methanol_tank, &methanol_tank_addr, None)).unwrap();

    // Loggers
    let process_logger = EventQueue::<ContProcessLog<Reactants>>::new();
    reaction_vessel.log_emitter.connect_sink(&process_logger);
    decantation_unit.log_emitter.connect_sink(&process_logger);
    drying_unit.log_emitter.connect_sink(&process_logger);

    let stock_logger = EventQueue::<ContStockLog<Reactants>>::new();
    methanol_tank.log_emitter.connect_sink(&stock_logger);
    crude_glycerin_tank.log_emitter.connect_sink(&stock_logger);
    oil_tank.log_emitter.connect_sink(&stock_logger);
    reaction_to_decant_stock.log_emitter.connect_sink(&stock_logger);
    decant_to_drying_stock.log_emitter.connect_sink(&stock_logger);
    product_stock.log_emitter.connect_sink(&stock_logger);
        

    // Simulation initialisation
    let sim_init = SimInit::new()
        .add_model(oil_tank, oil_tank_mbox, "OilTank")
        .add_model(methanol_tank, methanol_tank_mbox, "Methanol Tank")
        .add_model(crude_glycerin_tank, crude_glycerin_tank_mbox, "Crude Glycerin Tank")
        .add_model(reaction_vessel, reaction_vessel_mbox, "Reaction Vessel")
        .add_model(reaction_to_decant_stock, reaction_to_decant_stock_mbox, "Reaction to Decant Stock")
        .add_model(decantation_unit, decantation_unit_mbox, "Decantation Unit")
        .add_model(decant_to_drying_stock, decant_to_drying_stock_mbox, "Decant to Drying Stock")
        .add_model(drying_unit, drying_unit_mbox, "Drying Unit")
        .add_model(product_stock, product_stock_mbox, "Product Stock");
    let start_time = MonotonicTime::try_from_date_time(2025, 7, 1, 0, 0, 0, 0).unwrap();
    let duration = Duration::from_secs(3600);
    let (mut sim, _) = sim_init.init(start_time).unwrap();
    sim.step_until(start_time + duration).unwrap();

    for log in process_logger.into_reader() {
        println!("{:?}", log);
    } 
    for log in stock_logger.into_reader() {
        println!("{:?}", log);
    }
    
}