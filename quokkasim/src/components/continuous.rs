use nexosim::{
    model::Model,
    ports::{Output, Requestor},
};
use serde::{Serialize, ser::SerializeStruct};
use std::{fmt::Debug, time::Duration};
use tai_time::MonotonicTime;

use crate::{distributions::Distribution, prelude::*};

#[derive(Debug, Clone, Serialize)]
pub enum ContinuousStockState {
    Normal { occupied: f64, empty: f64 },
    Full { occupied: f64, empty: f64 },
    Empty { occupied: f64, empty: f64 },
}

impl StockState for ContinuousStockState {
    fn is_same_state(&self, other: &Self) -> bool {
        match (self, other) {
            (ContinuousStockState::Empty { .. }, ContinuousStockState::Empty { .. }) => true,
            (ContinuousStockState::Normal { .. }, ContinuousStockState::Normal { .. }) => true,
            (ContinuousStockState::Full { .. }, ContinuousStockState::Full { .. }) => true,
            _ => false,
        }
    }
}

impl<T: ContinuousResource> ToLogRecord<ContinuousStockLogType<T>, ContinuousStockLog<T>> for DefaultStock<T, ContinuousStockState, ContinuousStockLog<T>> {
    fn to_record(
        &mut self,
        now: MonotonicTime,
        source_event_id: EventId,
        event_id: EventId,
        details: ContinuousStockLogType<T>,
    ) -> ContinuousStockLog<T> {
        ContinuousStockLog::to_log(
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
pub struct DefaultStock<T, S: StockState, RecordLogType: Clone + Send + 'static>
where
    T: ContinuousArithmetic + Clone + Serialize + Send + 'static,
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

impl<T: ContinuousResource + 'static, S: StockState + Send + 'static, RecordLogType: Clone + Send + 'static> Model for DefaultStock<T, S, RecordLogType> {}

impl<T: ContinuousResource + Default + 'static, S: StockState, RecordLogType: Clone + Send + 'static> Default for DefaultStock<T, S, RecordLogType> {
    fn default() -> Self {
        let (log_emitter, state_emitter) = (Output::new(), Output::new());
        DefaultStock {
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
    ResourceType: ContinuousResource + 'static,
    LogRecordType: Clone + Send + 'static,
> Stock<
    ResourceType,
    ContinuousStockState,
    LogRecordType,
    ContinuousStockLogType<ResourceType>,
> for DefaultStock<ResourceType, ContinuousStockState, LogRecordType>
where
    LogRecordType: Serialize,
    Self: ToLogRecord<ContinuousStockLogType<ResourceType>, LogRecordType>,
{
    fn get_state(&mut self) -> ContinuousStockState {
        let occupied = self.resource.total();
        let empty = self.max_capacity - occupied;
        if empty <= 0.0 {
            ContinuousStockState::Full { occupied, empty }
        } else if occupied < self.low_capacity {
            ContinuousStockState::Empty { occupied, empty }
        } else {
            ContinuousStockState::Normal { occupied, empty }
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

    fn previous_state(&mut self) -> &mut Option<ContinuousStockState> { &mut self.prev_state }
    fn resource(&mut self) -> &mut ResourceType { &mut self.resource }
    fn state_emitter(&mut self) -> &mut Output<EventId> { &mut self.state_emitter }

    fn log_type_add(&self, balance: f64, resource: ResourceType) -> ContinuousStockLogType<ResourceType> {
        ContinuousStockLogType::Add { balance, resource }
    }
    fn log_type_remove(&self, balance: f64, resource: ResourceType) -> ContinuousStockLogType<ResourceType> {
        ContinuousStockLogType::Remove { balance, resource }
    }
    fn log_type_state_change(&self, new_state: ContinuousStockState) -> ContinuousStockLogType<ResourceType> {
        ContinuousStockLogType::StateChange { new_state }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ContinuousStockLog<T: ContinuousArithmetic> {
    pub time: String,
    pub event_id: EventId,
    pub source_event_id: EventId,
    pub element_name: String,
    pub element_type: String,
    pub details: ContinuousStockLogType<T>,
}

impl<T: ContinuousArithmetic + Debug + Serialize> ContinuousStockLog<T> {
    fn to_log(
        time: MonotonicTime,
        event_id: EventId,
        source_event_id: EventId,
        element_name: String,
        element_type: String,
        details: ContinuousStockLogType<T>,
    ) -> Self {
        ContinuousStockLog {
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
pub enum ContinuousStockLogType<T: ContinuousArithmetic> {
    Add { balance: f64, resource: T },
    Remove { balance: f64, resource: T },
    StateChange { new_state: ContinuousStockState },
}

#[derive(Debug, Clone)]
pub struct ContinuousProcessLog<LogDetailsType, ResourceType: ContinuousResource> {
    pub time: String,
    pub event_id: EventId,
    pub source_event_id: EventId,
    pub element_name: String,
    pub element_type: String,
    pub details: LogDetailsType,

    pub phantom: std::marker::PhantomData<ResourceType>,
}

impl<D, T: ContinuousResource> Serialize for ContinuousProcessLog<D, T>
where
    D: Clone + Into<DefaultProcessLogType<T>>,
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
        let details: DefaultProcessLogType<T> = self.details.clone().into();
        let (event_type, total, resource, reason): (&str, Option<f64>, Option<T>, Option<String>) =
            match details {
                DefaultProcessLogType::WithdrawRequest => ("WithdrawRequest", None, None, None),
                DefaultProcessLogType::ProcessStart { quantity, resource } => {
                    ("ProcessStart", Some(quantity), Some(resource.clone()), None)
                }
                DefaultProcessLogType::ProcessSuccess { quantity, resource } => {
                    ("ProcessSuccess", Some(quantity), Some(resource.clone()), None)
                }
                DefaultProcessLogType::ProcessFailure { reason } => {
                    ("ProcessFailure", None, None, Some(reason.to_string()))
                }
                DefaultProcessLogType::ProcessStopped { reason } => {
                    ("ProcessStopped", None, None, Some(reason.to_string()))
                }
                DefaultProcessLogType::ProcessContinue { reason } => {
                    ("ProcessContinue", None, None, Some(reason.to_string()))
                }
                DefaultProcessLogType::DelayStart { delay_name } => {
                    ("DelayStart", None, None, Some(delay_name.clone()))
                }
                DefaultProcessLogType::DelayEnd { delay_name } => {
                    ("DelayEnd", None, None, Some(delay_name.clone()))
                }
                DefaultProcessLogType::StateChange { new_state } => {
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
pub enum DefaultProcessLogType<T: ContinuousResource> {
    WithdrawRequest,
    ProcessStart { quantity: f64, resource: T },
    ProcessSuccess { quantity: f64, resource: T },
    ProcessFailure { reason: &'static str },
    ProcessStopped { reason: &'static str },
    ProcessContinue { reason: &'static str },
    DelayStart { delay_name: String },
    DelayEnd { delay_name: String },
    StateChange { new_state: ContinuousStockState },
}

// ──────────────────────────── DefaultProcess ────────────────────────────
/* #region DefaultProcess */

#[derive(WithMethods)]
pub struct DefaultProcess<
    ResourceType: Clone + Send + Debug + 'static,
    ProcessLog: Clone + Send + Debug + 'static,
> {
    // Identification
    pub element_name: String,
    pub element_code: String,
    pub element_type: String,

    // Ports
    pub req_upstream: Requestor<(), ContinuousStockState>,
    pub req_downstream: Requestor<(), ContinuousStockState>,
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
    ResourceType: ContinuousResource + 'static,
    ProcessLog: Clone + Send + Debug + Serialize + 'static,
> Default for DefaultProcess<ResourceType, ProcessLog>
{
    fn default() -> Self {
        DefaultProcess::<ResourceType, ProcessLog> {
            element_name: "DefaultProcess".into(),
            element_code: "".into(),
            element_type: "DefaultProcess".into(),

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

impl<ResourceType: ContinuousResource> Model
    for DefaultProcess<
        ResourceType,
        ContinuousProcessLog<DefaultProcessLogType<ResourceType>, ResourceType>,
    >
where
    ContinuousProcessLog<DefaultProcessLogType<ResourceType>, ResourceType>: Serialize,
    Self: ToLogRecord<
            DefaultProcessLogType<ResourceType>,
            ContinuousProcessLog<DefaultProcessLogType<ResourceType>, ResourceType>,
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

impl<ResourceType: ContinuousResource>
    ToLogRecord<
        DefaultProcessLogType<ResourceType>,
        ContinuousProcessLog<DefaultProcessLogType<ResourceType>, ResourceType>,
    >
    for DefaultProcess<
        ResourceType,
        ContinuousProcessLog<DefaultProcessLogType<ResourceType>, ResourceType>,
    >
{
    fn to_record(
        &mut self,
        now: MonotonicTime,
        source_event_id: EventId,
        event_id: EventId,
        details: DefaultProcessLogType<ResourceType>,
    ) -> ContinuousProcessLog<DefaultProcessLogType<ResourceType>, ResourceType> {
        ContinuousProcessLog::<DefaultProcessLogType<ResourceType>, ResourceType> {
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

impl<T: ContinuousResource + 'static>
    ProcessCore<T, ContinuousProcessLog<DefaultProcessLogType<T>, T>, DefaultProcessLogType<T>>
    for DefaultProcess<T, ContinuousProcessLog<DefaultProcessLogType<T>, T>>
where
    ContinuousProcessLog<DefaultProcessLogType<T>, T>: Serialize,
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
    fn log_emitter(&mut self) -> &mut Output<ContinuousProcessLog<DefaultProcessLogType<T>, T>> {
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

    fn log_type_withdraw_request(&self) -> DefaultProcessLogType<T> {
        DefaultProcessLogType::WithdrawRequest
    }
    fn log_type_process_start(&self, quantity: f64, resource: T) -> DefaultProcessLogType<T> {
        DefaultProcessLogType::ProcessStart { quantity, resource }
    }
    fn log_type_process_success(&self, quantity: f64, resource: T) -> DefaultProcessLogType<T> {
        DefaultProcessLogType::ProcessSuccess { quantity, resource }
    }
    fn log_type_process_failure(&self, reason: &'static str) -> DefaultProcessLogType<T> {
        DefaultProcessLogType::ProcessFailure { reason }
    }
    fn log_type_process_stopped(&self, reason: &'static str) -> DefaultProcessLogType<T> {
        DefaultProcessLogType::ProcessStopped { reason }
    }
    fn log_type_process_continue(&self, reason: &'static str) -> DefaultProcessLogType<T> {
        DefaultProcessLogType::ProcessContinue { reason }
    }
    fn log_type_delay_start(&self, delay_name: String) -> DefaultProcessLogType<T> {
        DefaultProcessLogType::DelayStart { delay_name }
    }
    fn log_type_delay_end(&self, delay_name: String) -> DefaultProcessLogType<T> {
        DefaultProcessLogType::DelayEnd { delay_name }
    }
}

impl<T: ContinuousResource + 'static>
    Process<T, ContinuousProcessLog<DefaultProcessLogType<T>, T>, DefaultProcessLogType<T>>
    for DefaultProcess<T, ContinuousProcessLog<DefaultProcessLogType<T>, T>>
where
    ContinuousProcessLog<DefaultProcessLogType<T>, T>: Serialize,
{
    fn req_upstream(&mut self) -> &mut Requestor<(), ContinuousStockState> {
        &mut self.req_upstream
    }
    fn withdraw_upstream(&mut self) -> &mut Requestor<(f64, EventId), T> {
        &mut self.withdraw_upstream
    }
    fn req_downstream(&mut self) -> &mut Requestor<(), ContinuousStockState> {
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

/* #endregion DefaultProcess */

// ──────────────────────────── DefaultSource ────────────────────────────
/* #region DefaultSource */

#[derive(WithMethods)]
pub struct DefaultSource<
    ResourceType: Clone + Send + Debug + 'static,
    ProcessLog: Clone + Send + Debug + 'static,
> {
    // Identification
    pub element_name: String,
    pub element_code: String,
    pub element_type: String,

    // Ports
    pub req_downstream: Requestor<(), ContinuousStockState>,
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
    ResourceType: ContinuousResource + 'static,
    ProcessLog: Clone + Send + Debug + Serialize + 'static,
> Default for DefaultSource<ResourceType, ProcessLog>
{
    fn default() -> Self {
        DefaultSource::<ResourceType, ProcessLog> {
            element_name: "DefaultSource".into(),
            element_code: "".into(),
            element_type: "DefaultSource".into(),

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

impl<ResourceType: ContinuousResource>
    Model
    for DefaultSource<
        ResourceType,
        ContinuousProcessLog<DefaultProcessLogType<ResourceType>, ResourceType>,
    >
where
    ContinuousProcessLog<DefaultProcessLogType<ResourceType>, ResourceType>: Serialize,
    Self: ToLogRecord<
            DefaultProcessLogType<ResourceType>,
            ContinuousProcessLog<DefaultProcessLogType<ResourceType>, ResourceType>,
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

impl<ResourceType: ContinuousResource>
    ToLogRecord<
        DefaultProcessLogType<ResourceType>,
        ContinuousProcessLog<DefaultProcessLogType<ResourceType>, ResourceType>,
    >
    for DefaultSource<
        ResourceType,
        ContinuousProcessLog<DefaultProcessLogType<ResourceType>, ResourceType>,
    >
{
    fn to_record(
        &mut self,
        now: MonotonicTime,
        source_event_id: EventId,
        event_id: EventId,
        details: DefaultProcessLogType<ResourceType>,
    ) -> ContinuousProcessLog<DefaultProcessLogType<ResourceType>, ResourceType> {
        ContinuousProcessLog::<DefaultProcessLogType<ResourceType>, ResourceType> {
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

impl<T: ContinuousResource + 'static>
    ProcessCore<T, ContinuousProcessLog<DefaultProcessLogType<T>, T>, DefaultProcessLogType<T>>
    for DefaultSource<T, ContinuousProcessLog<DefaultProcessLogType<T>, T>>
where
    ContinuousProcessLog<DefaultProcessLogType<T>, T>: Serialize,
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
    fn log_emitter(&mut self) -> &mut Output<ContinuousProcessLog<DefaultProcessLogType<T>, T>> {
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

    fn log_type_withdraw_request(&self) -> DefaultProcessLogType<T> {
        DefaultProcessLogType::WithdrawRequest
    }
    fn log_type_process_start(&self, quantity: f64, resource: T) -> DefaultProcessLogType<T> {
        DefaultProcessLogType::ProcessStart { quantity, resource }
    }
    fn log_type_process_success(&self, quantity: f64, resource: T) -> DefaultProcessLogType<T> {
        DefaultProcessLogType::ProcessSuccess { quantity, resource }
    }
    fn log_type_process_failure(&self, reason: &'static str) -> DefaultProcessLogType<T> {
        DefaultProcessLogType::ProcessFailure { reason }
    }
    fn log_type_process_stopped(&self, reason: &'static str) -> DefaultProcessLogType<T> {
        DefaultProcessLogType::ProcessStopped { reason }
    }
    fn log_type_process_continue(&self, reason: &'static str) -> DefaultProcessLogType<T> {
        DefaultProcessLogType::ProcessContinue { reason }
    }
    fn log_type_delay_start(&self, delay_name: String) -> DefaultProcessLogType<T> {
        DefaultProcessLogType::DelayStart { delay_name }
    }
    fn log_type_delay_end(&self, delay_name: String) -> DefaultProcessLogType<T> {
        DefaultProcessLogType::DelayEnd { delay_name }
    }
}


impl<T: ContinuousResource + 'static>
    Source<T, ContinuousProcessLog<DefaultProcessLogType<T>, T>, DefaultProcessLogType<T>>
    for DefaultSource<T, ContinuousProcessLog<DefaultProcessLogType<T>, T>>
where
    ContinuousProcessLog<DefaultProcessLogType<T>, T>: Serialize,
{
    fn req_downstream(&mut self) -> &mut Requestor<(), ContinuousStockState> {
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

/* #endregion DefaultSource */

// ──────────────────────────── DefaultSink ────────────────────────────
/* #region DefaultSink */

#[derive(WithMethods)]
pub struct DefaultSink<
    ResourceType: Clone + Send + Debug + 'static,
    ProcessLog: Clone + Send + Debug + 'static,
> {
    // Identification
    pub element_name: String,
    pub element_code: String,
    pub element_type: String,

    // Ports
    pub req_upstream: Requestor<(), ContinuousStockState>,
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
    ResourceType: ContinuousResource + 'static,
    ProcessLog: Clone + Send + Debug + Serialize + 'static,
> Default for DefaultSink<ResourceType, ProcessLog>
{
    fn default() -> Self {
        DefaultSink::<ResourceType, ProcessLog> {
            element_name: "DefaultSink".into(),
            element_code: "".into(),
            element_type: "DefaultSink".into(),

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

impl<ResourceType: ContinuousResource>
    Model
    for DefaultSink<
        ResourceType,
        ContinuousProcessLog<DefaultProcessLogType<ResourceType>, ResourceType>,
    >
where
    ContinuousProcessLog<DefaultProcessLogType<ResourceType>, ResourceType>: Serialize,
    Self: ToLogRecord<
            DefaultProcessLogType<ResourceType>,
            ContinuousProcessLog<DefaultProcessLogType<ResourceType>, ResourceType>,
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

impl<ResourceType: ContinuousResource>
    ToLogRecord<
        DefaultProcessLogType<ResourceType>,
        ContinuousProcessLog<DefaultProcessLogType<ResourceType>, ResourceType>,
    >
    for DefaultSink<
        ResourceType,
        ContinuousProcessLog<DefaultProcessLogType<ResourceType>, ResourceType>,
    >
{
    fn to_record(
        &mut self,
        now: MonotonicTime,
        source_event_id: EventId,
        event_id: EventId,
        details: DefaultProcessLogType<ResourceType>,
    ) -> ContinuousProcessLog<DefaultProcessLogType<ResourceType>, ResourceType> {
        ContinuousProcessLog::<DefaultProcessLogType<ResourceType>, ResourceType> {
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

impl<T: ContinuousResource + 'static>
    ProcessCore<T, ContinuousProcessLog<DefaultProcessLogType<T>, T>, DefaultProcessLogType<T>>
    for DefaultSink<T, ContinuousProcessLog<DefaultProcessLogType<T>, T>>
where
    ContinuousProcessLog<DefaultProcessLogType<T>, T>: Serialize,
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
    fn log_emitter(&mut self) -> &mut Output<ContinuousProcessLog<DefaultProcessLogType<T>, T>> {
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

    fn log_type_withdraw_request(&self) -> DefaultProcessLogType<T> {
        DefaultProcessLogType::WithdrawRequest
    }
    fn log_type_process_start(&self, quantity: f64, resource: T) -> DefaultProcessLogType<T> {
        DefaultProcessLogType::ProcessStart { quantity, resource }
    }
    fn log_type_process_success(&self, quantity: f64, resource: T) -> DefaultProcessLogType<T> {
        DefaultProcessLogType::ProcessSuccess { quantity, resource }
    }
    fn log_type_process_failure(&self, reason: &'static str) -> DefaultProcessLogType<T> {
        DefaultProcessLogType::ProcessFailure { reason }
    }
    fn log_type_process_stopped(&self, reason: &'static str) -> DefaultProcessLogType<T> {
        DefaultProcessLogType::ProcessStopped { reason }
    }
    fn log_type_process_continue(&self, reason: &'static str) -> DefaultProcessLogType<T> {
        DefaultProcessLogType::ProcessContinue { reason }
    }
    fn log_type_delay_start(&self, delay_name: String) -> DefaultProcessLogType<T> {
        DefaultProcessLogType::DelayStart { delay_name }
    }
    fn log_type_delay_end(&self, delay_name: String) -> DefaultProcessLogType<T> {
        DefaultProcessLogType::DelayEnd { delay_name }
    }
}


impl<T: ContinuousResource + 'static>
    Sink<T, ContinuousProcessLog<DefaultProcessLogType<T>, T>, DefaultProcessLogType<T>>
    for DefaultSink<T, ContinuousProcessLog<DefaultProcessLogType<T>, T>>
where
    ContinuousProcessLog<DefaultProcessLogType<T>, T>: Serialize,
{
    fn req_upstream(&mut self) -> &mut Requestor<(), ContinuousStockState> {
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

/* #endregion DefaultSource */