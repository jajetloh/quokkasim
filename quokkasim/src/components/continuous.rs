use nexosim::{
    model::Model,
    ports::{Output, Requestor},
};
use serde::{Deserialize, Serialize, ser::SerializeStruct};
use strum_macros::Display;
use std::{fmt::Debug, time::Duration};
use tai_time::MonotonicTime;

use crate::{distributions::Distribution, prelude::*};

#[derive(Debug, Display, Clone, Serialize, Deserialize)]
pub enum ContStockState {
    Normal { occupied: f64, empty: f64 },
    Full { occupied: f64, empty: f64 },
    Empty { occupied: f64, empty: f64 },
}

impl StockState for ContStockState {
    fn is_same_state(&self, other: &Self) -> bool {
        match (self, other) {
            (ContStockState::Empty { .. }, ContStockState::Empty { .. }) => true,
            (ContStockState::Normal { .. }, ContStockState::Normal { .. }) => true,
            (ContStockState::Full { .. }, ContStockState::Full { .. }) => true,
            _ => false,
        }
    }
}

impl ContStockState {
    pub fn occupied_frac(&self) -> f64 {
        match self {
            ContStockState::Normal { occupied, empty } => {
                occupied / (occupied + empty)
            }
            ContStockState::Full { .. } => 1.0,
            ContStockState::Empty { .. } => 0.0,
        }
    }
    pub fn occupied(&self) -> f64 {
        match self {
            ContStockState::Normal { occupied, .. } => *occupied,
            ContStockState::Full { occupied, .. } => *occupied,
            ContStockState::Empty { occupied, .. } => *occupied,
        }
    }
    pub fn empty(&self) -> f64 {
        match self {
            ContStockState::Normal { empty, .. } => *empty,
            ContStockState::Full { empty, .. } => *empty,
            ContStockState::Empty { empty, .. } => *empty,
        }
    }
}

#[derive(WithMethods)]
pub struct DefaultContStock<ResourceType, StockStateType: StockState, RecordLogType: Clone + Send + 'static>
where
    ResourceType: ContArithmetic + Clone + Serialize + Send + 'static,
{
    // Identification
    pub element_name: String,
    pub element_code: String,
    pub element_type: String,

    // Ports
    pub log_emitter: Output<RecordLogType>,
    pub state_emitter: Output<EventMetadata>,

    // Configuration
    pub low_capacity: f64,
    pub max_capacity: f64,

    // Runtime State
    pub resource: ResourceType,

    // Internals
    prev_state: Option<StockStateType>,
    previous_event: EventMetadata,
}

impl<ResourceType: ContResource + 'static, S: StockState + Send + 'static, RecordLogType: Clone + Send + 'static> Model for DefaultContStock<ResourceType, S, RecordLogType> {}

impl<ResourceType: ContResource + Default + 'static, S: StockState, RecordLogType: Clone + Send + 'static> Default for DefaultContStock<ResourceType, S, RecordLogType> {
    fn default() -> Self {
        let (log_emitter, state_emitter) = (Output::new(), Output::new());
        DefaultContStock {
            element_name: String::new(),
            element_code: String::new(),
            element_type: String::new(),
            log_emitter,
            state_emitter,
            low_capacity: 0.0,
            max_capacity: 0.0,
            resource: ResourceType::default(),
            prev_state: None,
            previous_event: EventMetadata::default(),
        }
    }
}

impl<
    ResourceType: ContResource + 'static,
> ContStock<
    ResourceType,
    ContStockState,
> for DefaultContStock<ResourceType, ContStockState, ContStockLog<ResourceType>>
{
    fn get_state(&mut self) -> ContStockState {
        let occupied = self.resource.total();
        let empty = self.max_capacity - occupied;
        if empty <= 0.0 {
            ContStockState::Full { occupied, empty }
        } else if occupied < self.low_capacity {
            ContStockState::Empty { occupied, empty }
        } else {
            ContStockState::Normal { occupied, empty }
        }
    }

    fn get_next_event_meta(&mut self) -> EventMetadata {
        self.previous_event.next()
    }

    fn previous_state(&mut self) -> &mut Option<ContStockState> { &mut self.prev_state }
    fn resource(&mut self) -> &mut ResourceType { &mut self.resource }
    fn state_emitter(&mut self) -> &mut Output<EventMetadata> { &mut self.state_emitter }

    fn log_type_add(&mut self, source_event_id: &mut EventMetadata, resource: ResourceType, cx: &mut Context<Self>) -> impl Future<Output = EventMetadata> {
        async move {
            let current_event = self.get_next_event_meta();
            let balance = self.resource().total();
            self.log_emitter.send(ContStockLog {
                time: cx.time().to_chrono_date_time(0).unwrap().to_string(),
                event_id: current_event.clone(),
                source_event_id: source_event_id.clone(),
                element_name: self.element_name.clone(),
                element_type: self.element_type.clone(),
                details: ContStockLogType::Add { balance, resource: resource.clone() },
            }).await;
            current_event
        }
    }
    fn log_type_remove(&mut self, source_event_id: &mut EventMetadata, resource: ResourceType, cx: &mut Context<Self>) -> impl Future<Output = EventMetadata> {
        async move {
            let current_event = self.get_next_event_meta();
            let balance = self.resource().total();
            self.log_emitter.send(ContStockLog {
                time: cx.time().to_chrono_date_time(0).unwrap().to_string(),
                event_id: current_event.clone(),
                source_event_id: source_event_id.clone(),
                element_name: self.element_name.clone(),
                element_type: self.element_type.clone(),
                details: ContStockLogType::Remove { balance, resource: resource.clone() },
            }).await;
            current_event
        }
    }
    fn log_type_state_change(&mut self, source_event_id: &mut EventMetadata, new_state: ContStockState, cx: &mut Context<Self>) -> impl Future<Output = EventMetadata> {
        async move {
            let current_event = self.get_next_event_meta();
            self.log_emitter.send(ContStockLog {
                time: cx.time().to_chrono_date_time(0).unwrap().to_string(),
                event_id: current_event.clone(),
                source_event_id: source_event_id.clone(),
                element_name: self.element_name.clone(),
                element_type: self.element_type.clone(),
                details: ContStockLogType::StateChange { new_state: new_state.clone() },
            }).await;
            current_event
        }
    }
}

#[derive(Debug, Clone)]
pub struct ContStockLog<ResourceType: ContArithmetic> {
    pub time: String,
    pub event_id: EventMetadata,
    pub source_event_id: EventMetadata,
    pub element_name: String,
    pub element_type: String,
    pub details: ContStockLogType<ResourceType>,
}

impl<ResourceType: ContArithmetic + Clone + Serialize> Serialize for ContStockLog<ResourceType> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer {
        let mut state = serializer.serialize_struct("ContinuousStockLog", 8)?;
        state.serialize_field("time", &self.time)?;
        state.serialize_field("event_id", &self.event_id)?;
        state.serialize_field("source_event_id", &self.source_event_id)?;
        state.serialize_field("element_name", &self.element_name)?;
        state.serialize_field("element_type", &self.element_type)?;
        let details: ContStockLogType<ResourceType> = self.details.clone().into();
        let (event_type, balance, resource): (&str, f64, Option<ResourceType>) =
            match details {
                ContStockLogType::Add { balance, resource } => ("Add", balance, Some(resource)),
                ContStockLogType::Remove { balance, resource } => ("Remove", balance, Some(resource)),
                ContStockLogType::StateChange { new_state  } => ("StateChange", new_state.occupied(), None),
            };
        state.serialize_field("event_type", &event_type)?;
        state.serialize_field("balance", &balance)?;
        state.serialize_field("resource", &resource)?;
        state.end()
    }
}

#[derive(Debug, Display, Clone, Serialize)]
pub enum ContStockLogType<ResourceType: ContArithmetic> {
    Add { balance: f64, resource: ResourceType },
    Remove { balance: f64, resource: ResourceType },
    StateChange { new_state: ContStockState },
}

#[derive(Debug, Clone)]
pub struct ContProcessLog<ResourceType: ContResource> {
    pub time: String,
    pub event_id: EventMetadata,
    pub source_event_id: EventMetadata,
    pub element_name: String,
    pub element_type: String,
    pub details: DefaultContProcessLogType<ResourceType>,
}

impl<ResourceType: ContResource> Serialize for ContProcessLog<ResourceType>
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("ContinuousProcessLog", 9)?;
        state.serialize_field("time", &self.time)?;
        state.serialize_field("event_id", &self.event_id)?;
        state.serialize_field("source_event_id", &self.source_event_id)?;
        state.serialize_field("element_name", &self.element_name)?;
        state.serialize_field("element_type", &self.element_type)?;
        let details: DefaultContProcessLogType<ResourceType> = self.details.clone().into();
        let (event_type, total, resource, reason): (&str, Option<f64>, Option<ResourceType>, Option<String>) =
            match details {
                DefaultContProcessLogType::WithdrawRequest { quantity } => ("WithdrawRequest", Some(quantity), None, None),
                DefaultContProcessLogType::ProcessStart { quantity, resource } => {
                    ("ProcessStart", Some(quantity), Some(resource.clone()), None)
                }
                DefaultContProcessLogType::ProcessSuccess { quantity, resource } => {
                    ("ProcessSuccess", Some(quantity), Some(resource.clone()), None)
                }
                DefaultContProcessLogType::ProcessFailure { reason } => {
                    ("ProcessFailure", None, None, Some(reason.to_string()))
                }
                DefaultContProcessLogType::ProcessStopped { reason } => {
                    ("ProcessStopped", None, None, Some(reason.to_string()))
                }
                DefaultContProcessLogType::ProcessContinue { reason } => {
                    ("ProcessContinue", None, None, Some(reason.to_string()))
                }
                DefaultContProcessLogType::StateChange { new_state } => {
                    ("StateChange", None, None, Some(format!("{:?}", new_state)))
                }
            };
        state.serialize_field("event_type", &event_type)?;
        state.serialize_field("total", &total)?;
        state.serialize_field("resource", &resource)?;
        state.serialize_field("reason", &reason)?;
        state.end()
    }
}

#[derive(Debug, Display, Clone)]
pub enum DefaultContProcessLogType<ResourceType: ContResource> {
    WithdrawRequest { quantity: f64 },
    ProcessStart { quantity: f64, resource: ResourceType },
    ProcessSuccess { quantity: f64, resource: ResourceType },
    ProcessFailure { reason: &'static str },
    ProcessStopped { reason: &'static str },
    ProcessContinue { reason: &'static str },
    StateChange { new_state: ContStockState },
}

// ──────────────────────────── DefaultContProcess ────────────────────────────
/* #region DefaultContProcess */

#[derive(WithMethods)]
pub struct DefaultContProcess<
    ResourceType: Clone + Send + Debug + 'static,
    ProcessLog: Clone + Send + Debug + 'static,
> {
    // Identification
    pub element_name: String,
    pub element_code: String,
    pub element_type: String,

    // Ports
    pub req_upstream: Requestor<(), ContStockState>,
    pub req_downstream: Requestor<(), ContStockState>,
    pub withdraw_upstream: Requestor<(f64, EventMetadata), ResourceType>,
    pub push_downstream: Output<(ResourceType, EventMetadata)>,
    pub log_emitter: Output<ProcessLog>,

    // Configuration
    pub process_quantity_distr: Distribution,
    pub process_time_distr: Distribution,

    // Runtime State
    pub process_state: Option<(Duration, ResourceType)>,

    // Internals
    pub time_to_next_process_event: Option<Duration>,
    pub scheduled_event: Option<(MonotonicTime, ActionKey)>,
    pub previous_event: EventMetadata,
    pub previous_check_time: MonotonicTime,
}

impl<
    ResourceType: ContResource + 'static,
    ProcessLog: Clone + Send + Debug + Serialize + 'static,
> Default for DefaultContProcess<ResourceType, ProcessLog>
{
    fn default() -> Self {
        DefaultContProcess::<ResourceType, ProcessLog> {
            element_name: "DefaultContProcess".into(),
            element_code: "".into(),
            element_type: "DefaultContProcess".into(),

            req_upstream: Requestor::default(),
            req_downstream: Requestor::default(),
            withdraw_upstream: Requestor::default(),
            push_downstream: Output::default(),
            log_emitter: Output::default(),

            process_state: None,

            process_quantity_distr: Distribution::default(),
            process_time_distr: Distribution::default(),

            time_to_next_process_event: None,
            scheduled_event: None,
            previous_event: EventMetadata::default(),
            previous_check_time: MonotonicTime::EPOCH,
        }
    }
}

impl<ResourceType: ContResource> Model
    for DefaultContProcess<
        ResourceType,
        ContProcessLog<ResourceType>,
    >
where
    ContProcessLog<ResourceType>: Serialize,
{
    fn init(
        mut self,
        ctx: &mut Context<Self>,
    ) -> impl Future<Output = InitializedModel<Self>> + Send {
        async move {
            self.previous_event = EventMetadata { source_name: self.element_name.clone(), source_code: self.element_code.clone(), index: 0 };
            self.update_state(self.previous_event.clone(), ctx).await;
            self.into()
        }
    }
}

impl<ResourceType: ContResource + 'static>
    ContProcessCore<ResourceType, ContProcessLog<ResourceType>>
    for DefaultContProcess<ResourceType, ContProcessLog<ResourceType>>
where
    ContProcessLog<ResourceType>: Serialize,
{
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

impl<ResourceType> ContProcessUpdateSinceLast<ResourceType, ContProcessLog<ResourceType>> for DefaultContProcess<ResourceType, ContProcessLog<ResourceType>>
where
    ResourceType: ContResource + 'static,
    Self: ContProcessCore<ResourceType, ContProcessLog<ResourceType>>,
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
                    self.time_to_next_process_event = None;
                } else {
                    self.process_state = Some((time_left, resource));
                    self.time_to_next_process_event = Some(time_left);
                }
            }
        }
    }

    fn log_type_process_success(&mut self, source_event_id: &mut EventMetadata, quantity: f64, resource: ResourceType, cx: &mut Context<Self>) -> impl Future<Output = EventMetadata> {
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

impl<ResourceType> ContProcessUpdateDecisionLogic<ResourceType, ContProcessLog<ResourceType>> for DefaultContProcess<ResourceType, ContProcessLog<ResourceType>>
where
    ResourceType: ContResource + 'static,
    Self: ContProcessCore<ResourceType, ContProcessLog<ResourceType>>,
{
    fn update_state_decision_logic(
            &mut self,
            source_event_id: &mut EventMetadata,
            cx: &mut Context<Self>,
        ) -> impl Future<Output = ()> {
        async move {
            match &self.process_state {
                None => {
                    let us_state = self.req_upstream.send(()).await.next();
                    let ds_state = self.req_downstream.send(()).await.next();

                    match (&us_state, &ds_state) {
                        (
                            Some(ContStockState::Normal { .. })
                            | Some(ContStockState::Full { .. }),
                            Some(ContStockState::Empty { .. })
                            | Some(ContStockState::Normal { .. }),
                        ) => {
                            let process_quantity = self.process_quantity_distr.sample();
                            *source_event_id = self.log_type_withdraw_request(source_event_id, process_quantity, cx).await;
                            let moved = self.withdraw_upstream.send((process_quantity, source_event_id.clone())).await.next().unwrap();
                            let process_duration_secs = self.process_time_distr.sample();
                            self.process_state = Some((
                                Duration::from_secs_f64(process_duration_secs),
                                moved.clone(),
                            ));
                            *source_event_id = self.log_type_process_start(source_event_id, process_quantity, moved.clone(), cx).await;
                            self.time_to_next_process_event = Some(Duration::from_secs_f64(process_duration_secs));
                        }
                        (Some(ContStockState::Empty { .. }), _) => {
                            *source_event_id = self.log_type_process_failure(source_event_id, "Upstream is empty", cx).await;
                            self.time_to_next_process_event = None;
                        }
                        (None, _) => {
                            *source_event_id = self.log_type_process_failure(source_event_id, "Upstream is not connected", cx).await;
                            self.time_to_next_process_event = None;
                        }
                        (_, None) => {
                            *source_event_id = self.log_type_process_failure(source_event_id, "Downstream is not connected", cx).await;
                            self.time_to_next_process_event = None;
                        }
                        (_, Some(ContStockState::Full { .. })) => {
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

    fn log_type_process_start(&mut self, source_event_id: &mut EventMetadata, quantity: f64, resource: ResourceType, cx: &mut Context<Self>) -> impl Future<Output = EventMetadata> {
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

impl<ResourceType> ContProcessUpdateForNextEvent<ResourceType, ContProcessLog<ResourceType>> for DefaultContProcess<ResourceType, ContProcessLog<ResourceType>>
where
    ResourceType: ContResource + 'static,
    Self: ContProcessCore<ResourceType, ContProcessLog<ResourceType>>,
{}

/* #endregion DefaultContProcess */

// ──────────────────────────── DefaultContSource ────────────────────────────
/* #region DefaultContSource */

#[derive(WithMethods)]
pub struct DefaultContSource<
    ResourceType: Clone + Send + Debug + 'static,
    ProcessLog: Clone + Send + Debug + 'static,
> {
    // Identification
    pub element_name: String,
    pub element_code: String,
    pub element_type: String,

    // Ports
    pub req_downstream: Requestor<(), ContStockState>,
    pub push_downstream: Output<(ResourceType, EventMetadata)>,
    pub log_emitter: Output<ProcessLog>,

    // Configuration
    pub source_resource: ResourceType,
    pub source_quantity_distr: Distribution,
    pub source_time_distr: Distribution,

    // Runtime State
    pub process_state: Option<(Duration, ResourceType)>,

    // Internals
    pub time_to_next_process_event: Option<Duration>,
    pub scheduled_event: Option<(MonotonicTime, ActionKey)>,
    pub previous_event: EventMetadata,
    pub previous_check_time: MonotonicTime,
}

impl<
    ResourceType: ContResource + 'static,
    ProcessLog: Clone + Send + Debug + Serialize + 'static,
> Default for DefaultContSource<ResourceType, ProcessLog>
{
    fn default() -> Self {
        DefaultContSource::<ResourceType, ProcessLog> {
            element_name: "DefaultContSource".into(),
            element_code: "".into(),
            element_type: "DefaultContSource".into(),

            req_downstream: Requestor::default(),
            push_downstream: Output::default(),
            log_emitter: Output::default(),

            process_state: None,

            source_resource: ResourceType::default(),
            source_quantity_distr: Distribution::default(),
            source_time_distr: Distribution::default(),

            time_to_next_process_event: None,
            scheduled_event: None,
            previous_event: EventMetadata::default(),
            previous_check_time: MonotonicTime::EPOCH,
        }
    }
}

impl<ResourceType: ContResource>
    Model
    for DefaultContSource<
        ResourceType,
        ContProcessLog<ResourceType>,
    >
where
    ContProcessLog<ResourceType>: Serialize,
{
    fn init(
        mut self,
        ctx: &mut Context<Self>,
    ) -> impl Future<Output = InitializedModel<Self>> + Send {
        async move {
            self.previous_event = EventMetadata { source_name: self.element_name.clone(), source_code: self.element_code.clone(), index: 0 };
            self.update_state(self.previous_event.clone(), ctx).await;
            self.into()
        }
    }
}

impl<ResourceType: ContResource + 'static>
    ContProcessCore<ResourceType, ContProcessLog<ResourceType>>
    for DefaultContSource<ResourceType, ContProcessLog<ResourceType>>
where
    ContProcessLog<ResourceType>: Serialize,
{

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


impl<ResourceType> ContProcessUpdateSinceLast<ResourceType, ContProcessLog<ResourceType>> for DefaultContSource<ResourceType, ContProcessLog<ResourceType>>
where
    ResourceType: ContResource + 'static,
    Self: ContProcessCore<ResourceType, ContProcessLog<ResourceType>>,
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
                    self.time_to_next_process_event = None;
                } else {
                    self.process_state = Some((time_left, resource));
                    self.time_to_next_process_event = Some(time_left);
                }
            }
        }
    }

    fn log_type_process_success(&mut self, source_event_id: &mut EventMetadata, quantity: f64, resource: ResourceType, cx: &mut Context<Self>) -> impl Future<Output = EventMetadata> {
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

impl<ResourceType> ContProcessUpdateDecisionLogic<
    ResourceType,
    ContProcessLog<ResourceType>,
> for DefaultContSource<ResourceType, ContProcessLog<ResourceType>>
where
    ResourceType: ContResource + 'static,
    Self: ContProcessCore<ResourceType, ContProcessLog<ResourceType>>,
{
    fn update_state_decision_logic(
            &mut self,
            source_event_id: &mut EventMetadata,
            cx: &mut Context<Self>,
        ) -> impl Future<Output = ()> {
        async move {
            match self.process_state {
                None => {
                    let ds_state = self.req_downstream.send(()).await.next();

                    match &ds_state {
                        Some(ContStockState::Empty { .. })
                        | Some(ContStockState::Normal { .. }) => {
                            let process_quantity = self.source_quantity_distr.sample();
                            let mut created_resource = self.source_resource.clone();
                            created_resource.multiply(process_quantity / self.source_resource.total());

                            *source_event_id = self.log_type_withdraw_request(source_event_id, process_quantity, cx).await;
                            let process_duration_secs = self.source_time_distr.sample();
                            self.process_state = Some((
                                Duration::from_secs_f64(process_duration_secs),
                                created_resource.clone()
                            ));
                            *source_event_id = self.log_type_process_start(source_event_id, process_quantity, created_resource.clone(), cx).await;
                            self.time_to_next_process_event = Some(Duration::from_secs_f64(process_duration_secs));
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

    fn log_type_process_start(&mut self, source_event_id: &mut EventMetadata, quantity: f64, resource: ResourceType, cx: &mut Context<Self>) -> impl Future<Output = EventMetadata> {
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

impl<ResourceType, LogRecordType> ContProcessUpdateForNextEvent<ResourceType, LogRecordType> for DefaultContSource<ResourceType, LogRecordType>
where
    ResourceType: ContResource + 'static,
    LogRecordType: Clone + Send + Debug + Serialize + 'static,
    Self: ContProcessCore<ResourceType, LogRecordType>,
{}

/* #endregion DefaultContSource */

// ──────────────────────────── DefaultContSink ────────────────────────────
/* #region DefaultContSink */

#[derive(WithMethods)]
pub struct DefaultContSink<
    ResourceType: Clone + Send + Debug + 'static,
    ProcessLog: Clone + Send + Debug + 'static,
> {
    // Identification
    pub element_name: String,
    pub element_code: String,
    pub element_type: String,

    // Ports
    pub req_upstream: Requestor<(), ContStockState>,
    pub withdraw_upstream: Requestor<(f64, EventMetadata), ResourceType>,
    pub log_emitter: Output<ProcessLog>,

    // Configuration
    pub sink_quantity_distr: Distribution,
    pub sink_time_distr: Distribution,

    // Runtime State
    pub process_state: Option<(Duration, ResourceType)>,

    // Internals
    pub time_to_next_process_event: Option<Duration>,
    pub scheduled_event: Option<(MonotonicTime, ActionKey)>,
    pub previous_event: EventMetadata,
    pub previous_check_time: MonotonicTime,
}

impl<
    ResourceType: ContResource + 'static,
    ProcessLog: Clone + Send + Debug + Serialize + 'static,
> Default for DefaultContSink<ResourceType, ProcessLog>
{
    fn default() -> Self {
        DefaultContSink::<ResourceType, ProcessLog> {
            element_name: "DefaultContSink".into(),
            element_code: "".into(),
            element_type: "DefaultContSink".into(),

            req_upstream: Requestor::default(),
            withdraw_upstream: Requestor::default(),
            log_emitter: Output::default(),

            process_state: None,

            sink_quantity_distr: Distribution::default(),
            sink_time_distr: Distribution::default(),

            time_to_next_process_event: None,
            scheduled_event: None,
            previous_event: EventMetadata::default(),
            previous_check_time: MonotonicTime::EPOCH,
        }
    }
}

impl<ResourceType: ContResource>
    Model
    for DefaultContSink<
        ResourceType,
        ContProcessLog<ResourceType>,
    >
where
    ContProcessLog<ResourceType>: Serialize,
{
    fn init(
        mut self,
        ctx: &mut Context<Self>,
    ) -> impl Future<Output = InitializedModel<Self>> + Send {
        async move {
            self.previous_event = EventMetadata { source_name: self.element_name.clone(), source_code: self.element_code.clone(), index: 0 };
            self.update_state(self.previous_event.clone(), ctx).await;
            self.into()
        }
    }
}

impl<ResourceType: ContResource + 'static>
    ContProcessCore<ResourceType, ContProcessLog<ResourceType>>
    for DefaultContSink<ResourceType, ContProcessLog<ResourceType>>
where
    ContProcessLog<ResourceType>: Serialize,
{
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

impl<ResourceType> ContProcessUpdateSinceLast<ResourceType, ContProcessLog<ResourceType>> for DefaultContSink<ResourceType, ContProcessLog<ResourceType>>
where
    ResourceType: ContResource + 'static,
    Self: ContProcessCore<ResourceType, ContProcessLog<ResourceType>>,
{
    fn update_process_state_since_prev_event(
        &mut self, source_event_id: &mut EventMetadata,
        cx: &mut Context<Self>,
        duration_since_prev: Duration
    ) -> impl Future<Output = ()> {
        async move {
            if let Some((mut process_time_left, resource)) = self.process_state.take() {
                process_time_left = process_time_left.saturating_sub(duration_since_prev);
                if process_time_left.is_zero() {
                    *source_event_id = self.log_type_process_success(source_event_id, resource.total(), resource.clone(), cx).await;
                } else {
                    self.process_state = Some((process_time_left, resource));
                }
            }
        }
    }

    fn log_type_process_success(&mut self, source_event_id: &mut EventMetadata, quantity: f64, resource: ResourceType, cx: &mut Context<Self>) -> impl Future<Output = EventMetadata> {
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

impl<ResourceType> ContProcessUpdateDecisionLogic<ResourceType, ContProcessLog<ResourceType>> for DefaultContSink<ResourceType, ContProcessLog<ResourceType>>
where
    ResourceType: ContResource + 'static,
    Self: ContProcessCore<ResourceType, ContProcessLog<ResourceType>>,
{
    fn update_state_decision_logic(
            &mut self,
            source_event_id: &mut EventMetadata,
            cx: &mut Context<Self>,
        ) -> impl Future<Output = ()> {
        async move {
            match self.process_state {
                None => {
                    let us_state = self.req_upstream.send(()).await.next();

                    match &us_state {
                        Some(ContStockState::Empty { .. })
                        | Some(ContStockState::Normal { .. }) => {
                            let process_quantity = self.sink_quantity_distr.sample();
                            let moved = self.withdraw_upstream.send((process_quantity, source_event_id.clone())).await.next().unwrap();

                            *source_event_id = self.log_type_withdraw_request(source_event_id, process_quantity, cx).await;
                            let process_duration_secs = self.sink_time_distr.sample();
                            self.process_state = Some((
                                Duration::from_secs_f64(process_duration_secs),
                                moved.clone()
                            ));
                            *source_event_id = self.log_type_process_start(source_event_id, process_quantity, moved.clone(), cx).await;
                            self.time_to_next_process_event = Some(Duration::from_secs_f64(process_duration_secs));
                        }
                        None => {
                            *source_event_id = self.log_type_process_failure(source_event_id, "Upstream is not connected", cx).await;
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

    fn log_type_process_start(&mut self, source_event_id: &mut EventMetadata, quantity: f64, resource: ResourceType, cx: &mut Context<Self>) -> impl Future<Output = EventMetadata> {
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

impl<ResourceType, LogRecordType> ContProcessUpdateForNextEvent<ResourceType, LogRecordType> for DefaultContSink<ResourceType, LogRecordType>
where
    ResourceType: ContResource + 'static,
    LogRecordType: Clone + Send + Debug + Serialize + 'static,
    Self: ContProcessCore<ResourceType, LogRecordType>,
{}
/* #endregion DefaultContSource */