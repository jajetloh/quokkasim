use std::time::Duration;
use std::fmt::Debug;

use nexosim::ports::Output;
use serde::Serialize;
use crate::prelude::*;

#[derive(WithMethods)]
pub struct DefaultLoadingProcess<
    ContainerType: Clone + Send + 'static,
    ContainerProcessLogType: Clone + Send + 'static,
    ResourceType: ContArithmetic + Clone + Serialize + Send + 'static,
> {
    // Identification
    pub element_name: String,
    pub element_code: String,
    pub element_type: String,

    // Ports
    pub req_upstream_vehicles: Requestor<(), DiscStockState>,
    pub withdraw_upstream_vehicles: Requestor<(usize, EventId), Vec<ContainerType>>,

    pub req_upstream_resources: Requestor<(), ContStockState>,
    pub withdraw_upstream_resources: Requestor<(usize, EventId), ResourceType>,

    pub req_downstream: Requestor<(), DiscStockState>,
    pub push_downstream: Output<(Vec<ContainerType>, EventId)>,

    pub log_emitter: Output<ContainerProcessLogType>,

    // Configuration
    pub process_quantity_distr: Distribution,
    pub process_time_distr: Distribution,

    // Runtime state
    pub process_state: Option<(Duration, Vec<ContainerType>)>,

    // Internals
    pub time_to_next_process_event: Option<Duration>,
    pub scheduled_event: Option<(MonotonicTime, ActionKey)>,
    pub next_event_index: u64,
    pub previous_check_time: MonotonicTime,
}

impl<
    ContainerType,
    ResourceType,
> Model for DefaultLoadingProcess<
    ContainerType,
    DiscProcessLog<ContainerType>,
    ResourceType,
> where
    ContainerType: LoadResource<ResourceType> + Clone + Debug + Serialize + Send + 'static,
    DiscProcessLog<ContainerType>: Clone + Debug + Serialize + Send + 'static,
    ResourceType: Projectable<f64> + ContResource + Clone + Debug + Serialize + Send + 'static,
{
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
            self.into()
        }
    }
}

// Core
impl<
    ContainerType,
    ResourceType,
> DiscProcessCore<
    ContainerType,
    DiscProcessLog<ContainerType>,
> for DefaultLoadingProcess<
    ContainerType,
    DiscProcessLog<ContainerType>,
    ResourceType,
>
where
    ContainerType: LoadResource<ResourceType> + Clone + Debug + Serialize + Send + 'static,
    DiscProcessLog<ContainerType>: Clone + Debug + Serialize + Send + 'static,
    ResourceType: Projectable<f64> + ContResource + Clone + Debug + Serialize + Send + 'static,
{
    fn update_state(
            &mut self,
            source_event_id: EventId,
            cx: &mut Context<Self>,
        ) -> impl Future<Output = ()> + Send {
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

    fn get_next_event_id(&mut self) -> EventId {
        let id = EventId(format!(
            "{}_{:06}",
            self.element_code, self.next_event_index
        ));
        self.next_event_index += 1;
        id
    }

    fn scheduled_event(
        &mut self,
    ) -> &mut Option<(MonotonicTime, ActionKey)> {
        &mut self.scheduled_event
    }

    fn previous_check_time(&mut self) -> &mut MonotonicTime {
        &mut self.previous_check_time
    }

    fn time_to_next_process_event(
        &mut self,
    ) -> &mut Option<Duration> {
        &mut self.time_to_next_process_event
    }
}

impl<
    ContainerType,
    ResourceType: ContArithmetic + Clone + Serialize + Send + 'static,  
> DiscProcessUpdateSinceLast<
    ContainerType,
    DiscProcessLog<ContainerType>,
> for DefaultLoadingProcess<
    ContainerType,
    DiscProcessLog<ContainerType>,
    ResourceType,
>
where
    ContainerType: Clone + Debug + Serialize + Send + 'static,
    DiscProcessLog<ContainerType>: Clone + Debug + Serialize + Send + 'static,
    ResourceType: ContResource + Clone + Debug + Serialize + Send + 'static,
    Self: DiscProcessCore<ContainerType, DiscProcessLog<ContainerType>>,
{
    fn update_process_state_since_prev_event(
        &mut self,
        source_event_id: &mut EventId,
        cx: &mut Context<Self>,
        duration_since_prev: Duration
    ) -> impl Future<Output = ()> {
        async move {
            if let Some((mut time_left, resources)) = self.process_state.take() {
                time_left = time_left.saturating_sub(duration_since_prev);
                if time_left.is_zero() {
                    let log_payload = resources.clone();
                    *source_event_id = self.log_type_process_success(source_event_id, 1, log_payload, cx).await;
                    self.push_downstream
                        .send((resources, source_event_id.clone()))
                        .await;
                    self.time_to_next_process_event = None;
                } else {
                    self.process_state = Some((time_left, resources));
                    self.time_to_next_process_event = Some(time_left);
                }
            }
        }
    }

    
    fn log_type_process_success(&mut self, source_event_id: &mut EventId, quantity: usize, resources: Vec<ContainerType>, cx: &mut Context<Self>) -> impl Future<Output = EventId> {
        async move {
            let current_event_id = self.get_next_event_id();
            self.log_emitter.send(DiscProcessLog {
                time: cx.time().to_chrono_date_time(0).unwrap().to_string(),
                event_id: current_event_id.clone(),
                source_event_id: source_event_id.clone(),
                element_name: self.element_name.clone(),
                element_type: self.element_type.clone(),
                details: DefaultDiscProcessLogType::ProcessSuccess { quantity, resources },
            }).await;
            current_event_id
        }
    }
}

pub trait LoadResource<ResourceType> where ResourceType: Projectable<f64> {
    fn load_resource(&mut self, resource: ResourceType);
    fn unload_resource(&mut self, amount: f64) -> ResourceType;
}

impl<
    ContainerType,
    ResourceType,
> DiscProcessUpdateDecisionLogic<
    ContainerType,
    DiscProcessLog<ContainerType>,
> for DefaultLoadingProcess<
    ContainerType,
    DiscProcessLog<ContainerType>,
    ResourceType,
>
where
    ContainerType: Clone + Debug + Serialize + Send + 'static,
    ResourceType: ContArithmetic + Clone + Debug + Serialize + Send + 'static + Projectable<f64>,
    ContainerType: LoadResource<ResourceType>,
    Self: DiscProcessCore<
        ContainerType,
        DiscProcessLog<ContainerType>,
    >
{
    fn update_state_decision_logic(
        &mut self,
        source_event_id: &mut EventId,
        cx: &mut Context<Self>,
    ) -> impl Future<Output = ()> {
        async move {
            match self.process_state {
                None => {
                    let upstream_resource_state = self.req_upstream_resources.send(()).await.next();
                    let upstream_vehicles_state = self.req_upstream_vehicles.send(()).await.next();

                    let downstream_state = self.req_downstream.send(()).await.next();

                    match (&upstream_resource_state, &upstream_vehicles_state, &downstream_state) {
                        (
                            Some(ContStockState::Normal { .. })
                            | Some(ContStockState::Full { .. }),
                            Some(DiscStockState::Normal { .. })
                            | Some(DiscStockState::Full { .. }),
                            Some(DiscStockState::Empty { .. })
                            | Some(DiscStockState::Normal { .. }),
                        ) => {
                            let mut requested =
                                self.process_quantity_distr.sample().round();
                            if !requested.is_finite() {
                                requested = 1.0;
                            }
                            let requested = requested.clamp(1.0, u32::MAX as f64) as usize;

                            *source_event_id = self.log_type_withdraw_request(source_event_id, requested, cx).await;

                            let resources_pulled = self.withdraw_upstream_resources.send((requested, source_event_id.clone())).await.next();
                            let vehicles_pulled = self.withdraw_upstream_vehicles.send((1, source_event_id.clone())).await.next();

                            match (resources_pulled, vehicles_pulled) {
                                (Some(batch), Some(mut vehicles)) => {
                                    if vehicles.len() != 1 {
                                        println!("Warning: Expected to withdraw 1 vehicle, but got {}", vehicles.len());
                                    }
                                    if vehicles.is_empty() {
                                        *source_event_id = self.log_type_process_failure(source_event_id, "Upstream returned no items", cx).await;
                                        self.time_to_next_process_event = None;
                                        return;
                                    }

                                    let mut process_secs =
                                        self.process_time_distr.sample();
                                    if !process_secs.is_finite() {
                                        process_secs = 0.0;
                                    }
                                    process_secs = process_secs.max(0.0);
                                    let mut process_duration =
                                        Duration::from_secs_f64(process_secs);
                                    if process_duration.is_zero() {
                                        process_duration = Duration::from_nanos(1);
                                    }

                                    vehicles.get_mut(0).unwrap().load_resource(batch.clone());

                                    self.process_state = Some((process_duration, vehicles.clone()));
                                    *source_event_id = self.log_type_process_start(source_event_id, vehicles.len(), vehicles.clone(), cx).await;
                                    self.time_to_next_process_event =
                                        Some(process_duration);
                                }
                                _ => {
                                    *source_event_id = self.log_type_process_failure(source_event_id, "Upstream requestor closed", cx).await;
                                    self.time_to_next_process_event = None;
                                }
                            }
                        }
                        (Some(ContStockState::Empty { .. }) | None, _, _) => {
                            *source_event_id = self.log_type_process_failure(source_event_id, "Upstream resource is empty or disconnected", cx).await;
                            self.time_to_next_process_event = None;
                        },
                        (_, Some(DiscStockState::Empty { .. }) | None, _) => {
                            *source_event_id = self.log_type_process_failure(source_event_id, "Upstream vehicles are empty", cx).await;
                            self.time_to_next_process_event = None;
                        }
                        (_, _, Some(DiscStockState::Full { .. }) | None) => {
                            *source_event_id = self.log_type_process_failure(source_event_id, "Downstream vehicles are full or disconnected", cx).await;
                            self.time_to_next_process_event = None;
                        }
                    }
                }
                Some((time_left, _)) => {
                    self.time_to_next_process_event = Some(time_left);
                }
            }
        }
    }

    fn log_type_withdraw_request(&mut self, source_event_id: &mut EventId, quantity: usize, cx: &mut Context<Self>) -> impl Future<Output = EventId> {
        async move {
            let current_event_id = self.get_next_event_id();
            self.log_emitter.send(DiscProcessLog {
                time: cx.time().to_chrono_date_time(0).unwrap().to_string(),
                event_id: current_event_id.clone(),
                source_event_id: source_event_id.clone(),
                element_name: self.element_name.clone(),
                element_type: self.element_type.clone(),
                details: DefaultDiscProcessLogType::WithdrawRequest { quantity },
            }).await;
            current_event_id
        }
    }
    
    fn log_type_process_start(&mut self, source_event_id: &mut EventId, quantity: usize, resources: Vec<ContainerType>, cx: &mut Context<Self>) -> impl Future<Output = EventId> {
        async move {
            let current_event_id = self.get_next_event_id();
            self.log_emitter.send(DiscProcessLog {
                time: cx.time().to_chrono_date_time(0).unwrap().to_string(),
                event_id: current_event_id.clone(),
                source_event_id: source_event_id.clone(),
                element_name: self.element_name.clone(),
                element_type: self.element_type.clone(),
                details: DefaultDiscProcessLogType::ProcessStart { quantity, resources },
            }).await;
            current_event_id
        }
    }

    fn log_type_process_failure(&mut self, source_event_id: &mut EventId, reason: &'static str, cx: &mut Context<Self>) -> impl Future<Output = EventId> {
        async move {
            let current_event_id = self.get_next_event_id();
            self.log_emitter.send(DiscProcessLog {
                time: cx.time().to_chrono_date_time(0).unwrap().to_string(),
                event_id: current_event_id.clone(),
                source_event_id: source_event_id.clone(),
                element_name: self.element_name.clone(),
                element_type: self.element_type.clone(),
                details: DefaultDiscProcessLogType::ProcessFailure { reason },
            }).await;
            current_event_id
        }
    }
}


impl<
    ContainerType,
    ResourceType,
> DiscProcessUpdateForNextEvent<
    ContainerType,
    DiscProcessLog<ContainerType>,
> for DefaultLoadingProcess<
    ContainerType,
    DiscProcessLog<ContainerType>,
    ResourceType,
> where
    ContainerType: Clone + Debug + Serialize + Send + 'static,
    DiscProcessLog<ContainerType>: Clone + Debug + Serialize + Send + 'static,
    ResourceType: ContResource + Clone + Debug + Serialize + Send + 'static,
    Self: DiscProcessCore<ContainerType, DiscProcessLog<ContainerType>>,
{}