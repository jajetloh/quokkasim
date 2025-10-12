use nexosim::{
    model::Model,
    ports::{Output, Requestor},
};
use serde::{Serialize, ser::SerializeStruct};
use std::{fmt::Debug, time::Duration};
use tai_time::MonotonicTime;

use crate::{distributions::Distribution, prelude::*};

#[derive(Debug, Clone, Serialize)]
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

impl<T: ContResource> ToLogRecord<ContStockLogType<T>, ContStockLog<T>> for DefaultContStock<T, ContStockState, ContStockLog<T>> {
    fn to_record(
        &mut self,
        now: MonotonicTime,
        source_event_id: EventId,
        event_id: EventId,
        details: ContStockLogType<T>,
    ) -> ContStockLog<T> {
        ContStockLog::to_log(
            now,
            event_id,
            source_event_id,
            self.element_name.clone(),
            self.element_type.clone(),
            details,
        )
    }
}

#[derive(WithMethods)]
pub struct DefaultContStock<T, S: StockState, RecordLogType: Clone + Send + 'static>
where
    T: ContArithmetic + Clone + Serialize + Send + 'static,
{
    // Identification
    pub element_name: String,
    pub element_code: String,
    pub element_type: String,

    // Ports
    pub log_emitter: Output<RecordLogType>,
    pub state_emitter: Output<EventId>,

    // Configuration
    pub low_capacity: f64,
    pub max_capacity: f64,

    // Runtime State
    pub resource: T,

    // Internals
    prev_state: Option<S>,
    next_event_index: u64,
}

impl<T: ContResource + 'static, S: StockState + Send + 'static, RecordLogType: Clone + Send + 'static> Model for DefaultContStock<T, S, RecordLogType> {}

impl<T: ContResource + Default + 'static, S: StockState, RecordLogType: Clone + Send + 'static> Default for DefaultContStock<T, S, RecordLogType> {
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
            resource: T::default(),
            prev_state: None,
            next_event_index: 0,
        }
    }
}

impl<
    ResourceType: ContResource + 'static,
    LogRecordType: Clone + Send + 'static,
> ContStock<
    ResourceType,
    ContStockState,
    LogRecordType,
    ContStockLogType<ResourceType>,
> for DefaultContStock<ResourceType, ContStockState, LogRecordType>
where
    LogRecordType: Serialize,
    Self: ToLogRecord<ContStockLogType<ResourceType>, LogRecordType>,
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

    fn log_emitter(&mut self) -> &mut Output<LogRecordType> {
        &mut self.log_emitter
    }

    fn get_next_event_id(&mut self) -> EventId {
        let event_id = EventId(format!(
            "{}_{:06}",
            self.element_code, self.next_event_index
        ));
        self.next_event_index += 1;
        event_id
    }

    fn previous_state(&mut self) -> &mut Option<ContStockState> { &mut self.prev_state }
    fn resource(&mut self) -> &mut ResourceType { &mut self.resource }
    fn state_emitter(&mut self) -> &mut Output<EventId> { &mut self.state_emitter }

    fn log_type_add(&self, balance: f64, resource: ResourceType) -> ContStockLogType<ResourceType> {
        ContStockLogType::Add { balance, resource }
    }
    fn log_type_remove(&self, balance: f64, resource: ResourceType) -> ContStockLogType<ResourceType> {
        ContStockLogType::Remove { balance, resource }
    }
    fn log_type_state_change(&self, new_state: ContStockState) -> ContStockLogType<ResourceType> {
        ContStockLogType::StateChange { new_state }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ContStockLog<T: ContArithmetic> {
    pub time: String,
    pub event_id: EventId,
    pub source_event_id: EventId,
    pub element_name: String,
    pub element_type: String,
    pub details: ContStockLogType<T>,
}

impl<T: ContArithmetic + Debug + Serialize> ContStockLog<T> {
    fn to_log(
        time: MonotonicTime,
        event_id: EventId,
        source_event_id: EventId,
        element_name: String,
        element_type: String,
        details: ContStockLogType<T>,
    ) -> Self {
        ContStockLog {
            time: time.to_chrono_date_time(0).unwrap().to_string(),
            event_id,
            source_event_id,
            element_name,
            element_type,
            details,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event_type")]
pub enum ContStockLogType<T: ContArithmetic> {
    Add { balance: f64, resource: T },
    Remove { balance: f64, resource: T },
    StateChange { new_state: ContStockState },
}

#[derive(Debug, Clone)]
pub struct ContProcessLog<LogDetailsType, ResourceType: ContResource> {
    pub time: String,
    pub event_id: EventId,
    pub source_event_id: EventId,
    pub element_name: String,
    pub element_type: String,
    pub details: LogDetailsType,

    pub phantom: std::marker::PhantomData<ResourceType>,
}

impl<D, T: ContResource> Serialize for ContProcessLog<D, T>
where
    D: Clone + Into<DefaultContProcessLogType<T>>,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("ContinuousProcessLog", 6)?;
        state.serialize_field("time", &self.time)?;
        state.serialize_field("event_id", &self.event_id)?;
        state.serialize_field("source_event_id", &self.source_event_id)?;
        state.serialize_field("element_name", &self.element_name)?;
        state.serialize_field("element_type", &self.element_type)?;
        let details: DefaultContProcessLogType<T> = self.details.clone().into();
        let (event_type, total, resource, reason): (&str, Option<f64>, Option<T>, Option<String>) =
            match details {
                DefaultContProcessLogType::WithdrawRequest => ("WithdrawRequest", None, None, None),
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
                DefaultContProcessLogType::DelayStart { delay_name } => {
                    ("DelayStart", None, None, Some(delay_name.clone()))
                }
                DefaultContProcessLogType::DelayEnd { delay_name } => {
                    ("DelayEnd", None, None, Some(delay_name.clone()))
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

#[derive(Debug, Clone)]
pub enum DefaultContProcessLogType<T: ContResource> {
    WithdrawRequest,
    ProcessStart { quantity: f64, resource: T },
    ProcessSuccess { quantity: f64, resource: T },
    ProcessFailure { reason: &'static str },
    ProcessStopped { reason: &'static str },
    ProcessContinue { reason: &'static str },
    DelayStart { delay_name: String },
    DelayEnd { delay_name: String },
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
    pub req_environment: Requestor<(), BasicEnvironmentState>,
    pub withdraw_upstream: Requestor<(f64, EventId), ResourceType>,
    pub push_downstream: Output<(ResourceType, EventId)>,
    pub log_emitter: Output<ProcessLog>,

    // Configuration
    pub process_quantity_distr: Distribution,
    pub process_time_distr: Distribution,
    pub delay_modes: DelayModes,

    // Runtime State
    pub process_state: Option<(Duration, ResourceType)>,
    pub env_state: BasicEnvironmentState,

    // Internals
    pub time_to_next_process_event: Option<Duration>,
    pub time_to_next_delay_event: Option<Duration>,
    pub scheduled_event: Option<(MonotonicTime, ActionKey)>,
    pub next_event_index: u64,
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
            req_environment: Requestor::default(),
            withdraw_upstream: Requestor::default(),
            push_downstream: Output::default(),
            log_emitter: Output::default(),

            process_state: None,
            env_state: BasicEnvironmentState::Normal,

            process_quantity_distr: Distribution::default(),
            process_time_distr: Distribution::default(),
            delay_modes: DelayModes::default(),

            time_to_next_process_event: None,
            time_to_next_delay_event: None,
            scheduled_event: None,
            next_event_index: 0,
            previous_check_time: MonotonicTime::EPOCH,
        }
    }
}

impl<ResourceType: ContResource> Model
    for DefaultContProcess<
        ResourceType,
        ContProcessLog<DefaultContProcessLogType<ResourceType>, ResourceType>,
    >
where
    ContProcessLog<DefaultContProcessLogType<ResourceType>, ResourceType>: Serialize,
    Self: ToLogRecord<
            DefaultContProcessLogType<ResourceType>,
            ContProcessLog<DefaultContProcessLogType<ResourceType>, ResourceType>,
        >,
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

impl<ResourceType: ContResource>
    ToLogRecord<
        DefaultContProcessLogType<ResourceType>,
        ContProcessLog<DefaultContProcessLogType<ResourceType>, ResourceType>,
    >
    for DefaultContProcess<
        ResourceType,
        ContProcessLog<DefaultContProcessLogType<ResourceType>, ResourceType>,
    >
{
    fn to_record(
        &mut self,
        now: MonotonicTime,
        source_event_id: EventId,
        event_id: EventId,
        details: DefaultContProcessLogType<ResourceType>,
    ) -> ContProcessLog<DefaultContProcessLogType<ResourceType>, ResourceType> {
        ContProcessLog::<DefaultContProcessLogType<ResourceType>, ResourceType> {
            time: now
                .to_chrono_date_time(0)
                .unwrap()
                .to_string(),
            event_id,
            source_event_id,
            element_name: self.element_name.clone(),
            element_type: self.element_type.clone(),
            details,
            phantom: std::marker::PhantomData,
        }
    }
}

impl<T: ContResource + 'static>
    ContProcessCore<T, ContProcessLog<DefaultContProcessLogType<T>, T>, DefaultContProcessLogType<T>>
    for DefaultContProcess<T, ContProcessLog<DefaultContProcessLogType<T>, T>>
where
    ContProcessLog<DefaultContProcessLogType<T>, T>: Serialize,
{
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
        let event_id = EventId(format!(
            "{}_{:06}",
            self.element_code, self.next_event_index
        ));
        self.next_event_index += 1;
        event_id
    }
    fn log_emitter(&mut self) -> &mut Output<ContProcessLog<DefaultContProcessLogType<T>, T>> {
        &mut self.log_emitter
    }
    fn scheduled_event(&mut self) -> &mut Option<(MonotonicTime, ActionKey)> {
        &mut self.scheduled_event
    }
    fn previous_check_time(&mut self) -> &mut MonotonicTime {
        &mut self.previous_check_time
    }
    fn delay_modes(&mut self) -> &mut DelayModes {
        &mut self.delay_modes
    }
    fn process_state(&mut self) -> &mut Option<(Duration, T)> {
        &mut self.process_state
    }
    fn env_state(&mut self) -> &mut BasicEnvironmentState {
        &mut self.env_state
    }
    fn req_environment(&mut self) -> &mut Requestor<(), BasicEnvironmentState> {
        &mut self.req_environment
    }
    fn time_to_next_process_event(&mut self) -> &mut Option<Duration> {
        &mut self.time_to_next_process_event
    }
    fn time_to_next_delay_event(&mut self) -> &mut Option<Duration> {
        &mut self.time_to_next_delay_event
    }

    fn log_type_withdraw_request(&self) -> DefaultContProcessLogType<T> {
        DefaultContProcessLogType::WithdrawRequest
    }
    fn log_type_process_start(&self, quantity: f64, resource: T) -> DefaultContProcessLogType<T> {
        DefaultContProcessLogType::ProcessStart { quantity, resource }
    }
    fn log_type_process_success(&self, quantity: f64, resource: T) -> DefaultContProcessLogType<T> {
        DefaultContProcessLogType::ProcessSuccess { quantity, resource }
    }
    fn log_type_process_failure(&self, reason: &'static str) -> DefaultContProcessLogType<T> {
        DefaultContProcessLogType::ProcessFailure { reason }
    }
    fn log_type_process_stopped(&self, reason: &'static str) -> DefaultContProcessLogType<T> {
        DefaultContProcessLogType::ProcessStopped { reason }
    }
    fn log_type_process_continue(&self, reason: &'static str) -> DefaultContProcessLogType<T> {
        DefaultContProcessLogType::ProcessContinue { reason }
    }
    fn log_type_delay_start(&self, delay_name: String) -> DefaultContProcessLogType<T> {
        DefaultContProcessLogType::DelayStart { delay_name }
    }
    fn log_type_delay_end(&self, delay_name: String) -> DefaultContProcessLogType<T> {
        DefaultContProcessLogType::DelayEnd { delay_name }
    }
}

impl<T: ContResource + 'static>
    ContProcess<T, ContProcessLog<DefaultContProcessLogType<T>, T>, DefaultContProcessLogType<T>>
    for DefaultContProcess<T, ContProcessLog<DefaultContProcessLogType<T>, T>>
where
    ContProcessLog<DefaultContProcessLogType<T>, T>: Serialize,
{
    fn req_upstream(&mut self) -> &mut Requestor<(), ContStockState> {
        &mut self.req_upstream
    }
    fn withdraw_upstream(&mut self) -> &mut Requestor<(f64, EventId), T> {
        &mut self.withdraw_upstream
    }
    fn req_downstream(&mut self) -> &mut Requestor<(), ContStockState> {
        &mut self.req_downstream
    }
    fn push_downstream(&mut self) -> &mut Output<(T, EventId)> {
        &mut self.push_downstream
    }
    fn process_quantity_distr(&mut self) -> &mut Distribution {
        &mut self.process_quantity_distr
    }
    fn process_time_distr(&mut self) -> &mut Distribution {
        &mut self.process_time_distr
    }
}

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
    pub req_environment: Requestor<(), BasicEnvironmentState>,
    pub push_downstream: Output<(ResourceType, EventId)>,
    pub log_emitter: Output<ProcessLog>,

    // Configuration
    pub source_resource: ResourceType,
    pub source_quantity_distr: Distribution,
    pub source_time_distr: Distribution,
    pub delay_modes: DelayModes,

    // Runtime State
    pub process_state: Option<(Duration, ResourceType)>,
    pub env_state: BasicEnvironmentState,

    // Internals
    pub time_to_next_process_event: Option<Duration>,
    pub time_to_next_delay_event: Option<Duration>,
    pub scheduled_event: Option<(MonotonicTime, ActionKey)>,
    pub next_event_index: u64,
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
            req_environment: Requestor::default(),
            push_downstream: Output::default(),
            log_emitter: Output::default(),

            process_state: None,
            env_state: BasicEnvironmentState::Normal,

            source_resource: ResourceType::default(),
            source_quantity_distr: Distribution::default(),
            source_time_distr: Distribution::default(),
            delay_modes: DelayModes::default(),

            time_to_next_process_event: None,
            time_to_next_delay_event: None,
            scheduled_event: None,
            next_event_index: 0,
            previous_check_time: MonotonicTime::EPOCH,
        }
    }
}

impl<ResourceType: ContResource>
    Model
    for DefaultContSource<
        ResourceType,
        ContProcessLog<DefaultContProcessLogType<ResourceType>, ResourceType>,
    >
where
    ContProcessLog<DefaultContProcessLogType<ResourceType>, ResourceType>: Serialize,
    Self: ToLogRecord<
            DefaultContProcessLogType<ResourceType>,
            ContProcessLog<DefaultContProcessLogType<ResourceType>, ResourceType>,
        >,
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

impl<ResourceType: ContResource>
    ToLogRecord<
        DefaultContProcessLogType<ResourceType>,
        ContProcessLog<DefaultContProcessLogType<ResourceType>, ResourceType>,
    >
    for DefaultContSource<
        ResourceType,
        ContProcessLog<DefaultContProcessLogType<ResourceType>, ResourceType>,
    >
{
    fn to_record(
        &mut self,
        now: MonotonicTime,
        source_event_id: EventId,
        event_id: EventId,
        details: DefaultContProcessLogType<ResourceType>,
    ) -> ContProcessLog<DefaultContProcessLogType<ResourceType>, ResourceType> {
        ContProcessLog::<DefaultContProcessLogType<ResourceType>, ResourceType> {
            time: now
                .to_chrono_date_time(0)
                .unwrap()
                .to_string(),
            event_id,
            source_event_id,
            element_name: self.element_name.clone(),
            element_type: self.element_type.clone(),
            details,
            phantom: std::marker::PhantomData,
        }
    }
}

impl<T: ContResource + 'static>
    ContProcessCore<T, ContProcessLog<DefaultContProcessLogType<T>, T>, DefaultContProcessLogType<T>>
    for DefaultContSource<T, ContProcessLog<DefaultContProcessLogType<T>, T>>
where
    ContProcessLog<DefaultContProcessLogType<T>, T>: Serialize,
{
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
        let event_id = EventId(format!(
            "{}_{:06}",
            self.element_code, self.next_event_index
        ));
        self.next_event_index += 1;
        event_id
    }
    fn log_emitter(&mut self) -> &mut Output<ContProcessLog<DefaultContProcessLogType<T>, T>> {
        &mut self.log_emitter
    }
    fn scheduled_event(&mut self) -> &mut Option<(MonotonicTime, ActionKey)> {
        &mut self.scheduled_event
    }
    fn previous_check_time(&mut self) -> &mut MonotonicTime {
        &mut self.previous_check_time
    }
    fn delay_modes(&mut self) -> &mut DelayModes {
        &mut self.delay_modes
    }
    fn process_state(&mut self) -> &mut Option<(Duration, T)> {
        &mut self.process_state
    }
    fn env_state(&mut self) -> &mut BasicEnvironmentState {
        &mut self.env_state
    }
    fn req_environment(&mut self) -> &mut Requestor<(), BasicEnvironmentState> {
        &mut self.req_environment
    }
    fn time_to_next_process_event(&mut self) -> &mut Option<Duration> {
        &mut self.time_to_next_process_event
    }
    fn time_to_next_delay_event(&mut self) -> &mut Option<Duration> {
        &mut self.time_to_next_delay_event
    }

    fn log_type_withdraw_request(&self) -> DefaultContProcessLogType<T> {
        DefaultContProcessLogType::WithdrawRequest
    }
    fn log_type_process_start(&self, quantity: f64, resource: T) -> DefaultContProcessLogType<T> {
        DefaultContProcessLogType::ProcessStart { quantity, resource }
    }
    fn log_type_process_success(&self, quantity: f64, resource: T) -> DefaultContProcessLogType<T> {
        DefaultContProcessLogType::ProcessSuccess { quantity, resource }
    }
    fn log_type_process_failure(&self, reason: &'static str) -> DefaultContProcessLogType<T> {
        DefaultContProcessLogType::ProcessFailure { reason }
    }
    fn log_type_process_stopped(&self, reason: &'static str) -> DefaultContProcessLogType<T> {
        DefaultContProcessLogType::ProcessStopped { reason }
    }
    fn log_type_process_continue(&self, reason: &'static str) -> DefaultContProcessLogType<T> {
        DefaultContProcessLogType::ProcessContinue { reason }
    }
    fn log_type_delay_start(&self, delay_name: String) -> DefaultContProcessLogType<T> {
        DefaultContProcessLogType::DelayStart { delay_name }
    }
    fn log_type_delay_end(&self, delay_name: String) -> DefaultContProcessLogType<T> {
        DefaultContProcessLogType::DelayEnd { delay_name }
    }
}


impl<T: ContResource + 'static>
    Source<T, ContProcessLog<DefaultContProcessLogType<T>, T>, DefaultContProcessLogType<T>>
    for DefaultContSource<T, ContProcessLog<DefaultContProcessLogType<T>, T>>
where
    ContProcessLog<DefaultContProcessLogType<T>, T>: Serialize,
{
    fn req_downstream(&mut self) -> &mut Requestor<(), ContStockState> {
        &mut self.req_downstream
    }
    fn push_downstream(&mut self) -> &mut Output<(T, EventId)> {
        &mut self.push_downstream
    }
    fn source_quantity_distr(&mut self) -> &mut Distribution {
        &mut self.source_quantity_distr
    }
    fn source_time_distr(&mut self) -> &mut Distribution {
        &mut self.source_time_distr
    }
    fn source_resource(&mut self) -> &mut T {
        &mut self.source_resource
    }
}

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
    pub req_environment: Requestor<(), BasicEnvironmentState>,
    pub withdraw_upstream: Requestor<(f64, EventId), ResourceType>,
    pub log_emitter: Output<ProcessLog>,

    // Configuration
    pub sink_quantity_distr: Distribution,
    pub sink_time_distr: Distribution,
    pub delay_modes: DelayModes,

    // Runtime State
    pub process_state: Option<(Duration, ResourceType)>,
    pub env_state: BasicEnvironmentState,

    // Internals
    pub time_to_next_process_event: Option<Duration>,
    pub time_to_next_delay_event: Option<Duration>,
    pub scheduled_event: Option<(MonotonicTime, ActionKey)>,
    pub next_event_index: u64,
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
            req_environment: Requestor::default(),
            withdraw_upstream: Requestor::default(),
            log_emitter: Output::default(),

            process_state: None,
            env_state: BasicEnvironmentState::Normal,

            sink_quantity_distr: Distribution::default(),
            sink_time_distr: Distribution::default(),
            delay_modes: DelayModes::default(),

            time_to_next_process_event: None,
            time_to_next_delay_event: None,
            scheduled_event: None,
            next_event_index: 0,
            previous_check_time: MonotonicTime::EPOCH,
        }
    }
}

impl<ResourceType: ContResource>
    Model
    for DefaultContSink<
        ResourceType,
        ContProcessLog<DefaultContProcessLogType<ResourceType>, ResourceType>,
    >
where
    ContProcessLog<DefaultContProcessLogType<ResourceType>, ResourceType>: Serialize,
    Self: ToLogRecord<
            DefaultContProcessLogType<ResourceType>,
            ContProcessLog<DefaultContProcessLogType<ResourceType>, ResourceType>,
        >
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

impl<ResourceType: ContResource>
    ToLogRecord<
        DefaultContProcessLogType<ResourceType>,
        ContProcessLog<DefaultContProcessLogType<ResourceType>, ResourceType>,
    >
    for DefaultContSink<
        ResourceType,
        ContProcessLog<DefaultContProcessLogType<ResourceType>, ResourceType>,
    >
{
    fn to_record(
        &mut self,
        now: MonotonicTime,
        source_event_id: EventId,
        event_id: EventId,
        details: DefaultContProcessLogType<ResourceType>,
    ) -> ContProcessLog<DefaultContProcessLogType<ResourceType>, ResourceType> {
        ContProcessLog::<DefaultContProcessLogType<ResourceType>, ResourceType> {
            time: now
                .to_chrono_date_time(0)
                .unwrap()
                .to_string(),
            event_id,
            source_event_id,
            element_name: self.element_name.clone(),
            element_type: self.element_type.clone(),
            details,
            phantom: std::marker::PhantomData,
        }
    }
}

impl<T: ContResource + 'static>
    ContProcessCore<T, ContProcessLog<DefaultContProcessLogType<T>, T>, DefaultContProcessLogType<T>>
    for DefaultContSink<T, ContProcessLog<DefaultContProcessLogType<T>, T>>
where
    ContProcessLog<DefaultContProcessLogType<T>, T>: Serialize,
{
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
        let event_id = EventId(format!(
            "{}_{:06}",
            self.element_code, self.next_event_index
        ));
        self.next_event_index += 1;
        event_id
    }
    fn log_emitter(&mut self) -> &mut Output<ContProcessLog<DefaultContProcessLogType<T>, T>> {
        &mut self.log_emitter
    }
    fn scheduled_event(&mut self) -> &mut Option<(MonotonicTime, ActionKey)> {
        &mut self.scheduled_event
    }
    fn previous_check_time(&mut self) -> &mut MonotonicTime {
        &mut self.previous_check_time
    }
    fn delay_modes(&mut self) -> &mut DelayModes {
        &mut self.delay_modes
    }
    fn process_state(&mut self) -> &mut Option<(Duration, T)> {
        &mut self.process_state
    }
    fn env_state(&mut self) -> &mut BasicEnvironmentState {
        &mut self.env_state
    }
    fn req_environment(&mut self) -> &mut Requestor<(), BasicEnvironmentState> {
        &mut self.req_environment
    }
    fn time_to_next_process_event(&mut self) -> &mut Option<Duration> {
        &mut self.time_to_next_process_event
    }
    fn time_to_next_delay_event(&mut self) -> &mut Option<Duration> {
        &mut self.time_to_next_delay_event
    }

    fn log_type_withdraw_request(&self) -> DefaultContProcessLogType<T> {
        DefaultContProcessLogType::WithdrawRequest
    }
    fn log_type_process_start(&self, quantity: f64, resource: T) -> DefaultContProcessLogType<T> {
        DefaultContProcessLogType::ProcessStart { quantity, resource }
    }
    fn log_type_process_success(&self, quantity: f64, resource: T) -> DefaultContProcessLogType<T> {
        DefaultContProcessLogType::ProcessSuccess { quantity, resource }
    }
    fn log_type_process_failure(&self, reason: &'static str) -> DefaultContProcessLogType<T> {
        DefaultContProcessLogType::ProcessFailure { reason }
    }
    fn log_type_process_stopped(&self, reason: &'static str) -> DefaultContProcessLogType<T> {
        DefaultContProcessLogType::ProcessStopped { reason }
    }
    fn log_type_process_continue(&self, reason: &'static str) -> DefaultContProcessLogType<T> {
        DefaultContProcessLogType::ProcessContinue { reason }
    }
    fn log_type_delay_start(&self, delay_name: String) -> DefaultContProcessLogType<T> {
        DefaultContProcessLogType::DelayStart { delay_name }
    }
    fn log_type_delay_end(&self, delay_name: String) -> DefaultContProcessLogType<T> {
        DefaultContProcessLogType::DelayEnd { delay_name }
    }
}


impl<T: ContResource + 'static>
    Sink<T, ContProcessLog<DefaultContProcessLogType<T>, T>, DefaultContProcessLogType<T>>
    for DefaultContSink<T, ContProcessLog<DefaultContProcessLogType<T>, T>>
where
    ContProcessLog<DefaultContProcessLogType<T>, T>: Serialize,
{
    fn req_upstream(&mut self) -> &mut Requestor<(), ContStockState> {
        &mut self.req_upstream
    }
    fn withdraw_upstream(&mut self) -> &mut Requestor<(f64, EventId), T> {
        &mut self.withdraw_upstream
    }
    fn sink_quantity_distr(&mut self) -> &mut Distribution {
        &mut self.sink_quantity_distr
    }
    fn sink_time_distr(&mut self) -> &mut Distribution {
        &mut self.sink_time_distr
    }
}

/* #endregion DefaultContSource */