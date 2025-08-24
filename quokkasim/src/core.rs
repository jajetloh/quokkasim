use std::{fmt::Debug, time::Duration};
use quokkasim_derive_macros::WithMethods;
use serde::{ser::SerializeStruct, Deserialize, Serialize};

use crate::{common::Distribution, delays::{DelayModes, DelayModeChange}, nexosim::{Output, Requestor, ActionKey, MonotonicTime, Address, Context, Model, InitializedModel}};

#[derive(Debug, Clone, Serialize, Deserialize)]
/// A short, lightweight identifier for an event. Very useful for understanding causal flow of events via log files.
/// Conventionally of the form `PROC_123456`, with a prefix uniquely identifying the process, and suffix an auto-incrementing number.
pub struct EventId(pub String);

impl EventId {
    pub fn from_init() -> EventId {
        EventId("INIT_000000".to_string())
    }

    pub fn from_scheduler() -> EventId {
        EventId("SCH_000000".to_string())
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum BasicEnvironmentState {
    Normal,
    Stopped,
}

#[derive(Clone)]
pub struct BasicEnvironmentLog {
    pub time: String,
    pub event_id: EventId,
    pub source_event_id: EventId,
    pub element_name: String,
    pub element_type: String,
    pub event: BasicEnvironmentState,
}

#[derive(WithMethods)]
pub struct BasicEnvironment {
    pub element_name: String,
    pub element_code: String,
    pub state: BasicEnvironmentState,
    pub next_event_index: u64,
    pub log_emitter: Output<BasicEnvironmentLog>,
    pub emit_change: Output<EventId>,
}

impl Default for BasicEnvironment {
    fn default() -> Self {
        BasicEnvironment {
            element_name: "BasicEnvironment".to_string(),
            element_code: "BE_000000".to_string(),
            state: BasicEnvironmentState::Normal,
            next_event_index: 0,
            log_emitter: Output::new(),
            emit_change: Output::new(),
        }
    }
}

impl Model for BasicEnvironment {
    fn init(mut self, cx: &mut Context<Self>) -> impl Future<Output = InitializedModel<Self>> + Send {
        async move {
            self.log(cx.time(), EventId::from_init(), self.state.clone()).await;
            self.into()
        }
    }
}

impl BasicEnvironment {
    pub fn with_state(mut self, state: BasicEnvironmentState) -> Self {
        self.state = state;
        self
    }

    pub fn set_state(&mut self, payload: (BasicEnvironmentState, EventId), cx: &mut Context<Self>) -> impl Future<Output = ()> {
        async move {
            let (state, mut event_id) = payload;
            if self.state != state {
                self.state = state;
                event_id = self.log(cx.time(), event_id, self.state.clone()).await;
                self.emit_change.send(event_id).await;
            }
        }
    }

    pub fn get_state_async(&mut self) -> impl Future<Output = BasicEnvironmentState> + {
        async move {
            self.state.clone()
        }
    }

    fn log(&mut self, now: MonotonicTime, source_event_id: EventId, event: BasicEnvironmentState) -> impl Future<Output = EventId> + Send {
        async move {
            let new_event_id = EventId(format!("{}_{:06}", self.element_code, self.next_event_index));
            let log = BasicEnvironmentLog {
                time: now.to_string(),
                event_id: new_event_id.clone(),
                source_event_id,
                element_name: self.element_name.clone(),
                element_type: "BasicEnvironment".to_string(),
                event,
            };
            self.log_emitter.send(log).await;
            self.next_event_index += 1;

            new_event_id
        }
    }
}


#[derive(Debug, Clone, Serialize)]
pub enum VectorStockState {
    Normal { occupied: f64, empty: f64 },
    Full { occupied: f64, empty: f64 },
    Empty { occupied: f64, empty: f64 },
}

impl StockState for VectorStockState {
    fn is_same_state(&self, other: &Self) -> bool {
        match (self, other) {
            (VectorStockState::Empty { .. }, VectorStockState::Empty { .. }) => true,
            (VectorStockState::Normal { .. }, VectorStockState::Normal { .. }) => true,
            (VectorStockState::Full { .. }, VectorStockState::Full { .. }) => true,
            _ => false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct VectorProcessLog<
    T: ContinuousResource,
> {
    pub time: String,
    pub event_id: EventId,
    pub source_event_id: EventId,
    pub element_name: String,
    pub element_type: String,
    pub details: VectorProcessLogType<T>,
}

impl<T: ContinuousResource> Serialize for VectorProcessLog<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("VectorProcessLog", 6)?;
        state.serialize_field("time", &self.time)?;
        state.serialize_field("event_id", &self.event_id)?;
        state.serialize_field("source_event_id", &self.source_event_id)?;
        state.serialize_field("element_name", &self.element_name)?;
        state.serialize_field("element_type", &self.element_type)?;
        let (event_type, total, resource, reason): (&str, Option<f64>, Option<T>, Option<String>) = match &self.details {
            VectorProcessLogType::WithdrawRequest => ("WithdrawRequest", None, None, None),
            VectorProcessLogType::ProcessStart { quantity, vector } => ("ProcessStart", Some(*quantity), Some(vector.clone()), None),
            VectorProcessLogType::ProcessSuccess { quantity, vector } => ("ProcessSuccess", Some(*quantity), Some(vector.clone()), None),
            VectorProcessLogType::ProcessFailure { reason } => ("ProcessFailure", None, None, Some(reason.to_string())),
            VectorProcessLogType::ProcessStopped { reason } => ("ProcessStopped", None, None, Some(reason.to_string())),
            VectorProcessLogType::ProcessContinue { reason } => ("ProcessContinue", None, None, Some(reason.to_string())),
            VectorProcessLogType::DelayStart { delay_name } => ("DelayStart", None, None, Some(delay_name.clone())),
            VectorProcessLogType::DelayEnd { delay_name } => ("DelayEnd", None, None, Some(delay_name.clone())),
            VectorProcessLogType::StateChange { new_state } => ("StateChange", None, None, Some(format!("{:?}", new_state))),
        };
        state.serialize_field("event_type", &event_type)?;
        state.serialize_field("total", &total)?;
        state.serialize_field("resource", &resource)?;
        state.serialize_field("reason", &reason)?;
        state.end()
    }
}

#[derive(Debug, Clone)]
pub enum VectorProcessLogType<T: ContinuousArithmetic> {
    WithdrawRequest,
    ProcessStart { quantity: f64, vector: T },
    ProcessSuccess { quantity: f64, vector: T },
    ProcessFailure { reason: &'static str },
    ProcessStopped { reason: &'static str },
    ProcessContinue { reason: &'static str },
    DelayStart { delay_name: String },
    DelayEnd { delay_name: String },
    StateChange { new_state: VectorStockState },
}

impl<T: ContinuousResource> VectorProcessLog<T> {
    pub fn to_log(
            time: MonotonicTime,
            event_id: EventId,
            source_event_id: EventId,
            element_name: String,
            element_type: String,
            details: VectorProcessLogType<T>,
        ) -> Self {
        VectorProcessLog {
            time: time.to_chrono_date_time(0).unwrap().to_string(),
            event_id,
            source_event_id,
            element_name,
            element_type,
            details,
        }
    }
}

#[derive(WithMethods)]
pub struct DefaultProcess<
    ResourceType: Clone + Send + Debug + 'static,
    ProcessLog: Clone + Send + Debug + Serialize + 'static,
> {
    // Identification
    pub element_name: String,
    pub element_code: String,
    pub element_type: String,
    
    // Ports
    pub req_upstream: Requestor<(), VectorStockState>,
    pub req_downstream: Requestor<(), VectorStockState>,
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
    ProcessLog: Clone + Send + Debug + Serialize + 'static
> Default for DefaultProcess<
    ResourceType,
    ProcessLog
> {
    fn default() -> Self {
        DefaultProcess {
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

impl<
    ResourceType: ContinuousResource,
> Model for DefaultProcess<
    ResourceType,
    VectorProcessLog<ResourceType>,
> {
    fn init(mut self, ctx: &mut Context<Self>) -> impl Future<Output = InitializedModel<Self>> + Send {
        async move {
            let source_event_id = EventId(format!("{}_{:06}", self.element_code, self.next_event_index));
            self.update_state(source_event_id, ctx).await;
            self.into()
        }
    }
}

impl<ResourceType: ContinuousResource> ToLogRecord<VectorProcessLogType<ResourceType>, VectorProcessLog<ResourceType>> for DefaultProcess<ResourceType, VectorProcessLog<ResourceType>> {
    fn to_record(&mut self, source_event_id: EventId, event_id: EventId, details: VectorProcessLogType<ResourceType>) -> VectorProcessLog<ResourceType> {
        VectorProcessLog::to_log(self.previous_check_time, event_id, source_event_id, self.element_name.clone(), self.element_type.clone(), details)
    }
}

impl<
    T: ContinuousResource + 'static
> Process<T, VectorProcessLog<T>> for DefaultProcess<T, VectorProcessLog<T>> {
    fn element_name(&self) -> &str { &self.element_name }
    fn element_code(&self) -> &str { &self.element_code }
    fn element_type(&self) -> &str { &self.element_type }
    fn get_next_event_id(&mut self) -> EventId {
        let event_id = EventId(format!("{}_{:06}", self.element_code, self.next_event_index));
        self.next_event_index += 1;
        event_id
    }
    fn log_emitter(&mut self) -> &mut Output<VectorProcessLog<T>> {
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
    fn req_upstream(&mut self) -> &mut Requestor<(), VectorStockState> {
        &mut self.req_upstream
    }
    fn withdraw_upstream(&mut self) -> &mut Requestor<(f64, EventId), T> {
        &mut self.withdraw_upstream
    }
    fn req_downstream(&mut self) -> &mut Requestor<(), VectorStockState> {
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
    fn time_to_next_process_event(&mut self) -> &mut Option<Duration> {
        &mut self.time_to_next_process_event
    }
    fn time_to_next_delay_event(&mut self) -> &mut Option<Duration> {
        &mut self.time_to_next_delay_event
    }
}

pub trait ToLogRecord<DetailsType, LogType> {
    fn to_record(&mut self, source_event_id: EventId, event_id: EventId, details: DetailsType) -> LogType;
}

pub trait Process<
    ResourceType: ContinuousResource + 'static,
    LogRecordType: Clone + Send + 'static,
> where Self: Model,
        Self: ToLogRecord<VectorProcessLogType<ResourceType>, LogRecordType> {

    fn update_state(
        &mut self, mut source_event_id: EventId, mut cx: &mut Context<Self>
    ) -> impl Future<Output = ()> + Send {
        async move {
            self.update_state_since_last_update(&mut source_event_id, &mut cx).await;
            self.update_state_decision_logic(&mut source_event_id, &mut cx).await;
            self.update_state_next_event(&mut source_event_id, &mut cx).await;
        }
    }

    fn update_state_since_last_update(&mut self, source_event_id: &mut EventId, cx: &mut Context<Self>) -> impl Future<Output = ()> + Send {
        async move {
            println!("Updating process state for {} at time {}", self.element_name(), cx.time());
            // Update variables from elapsed time
            if let Some((scheduled_time, _)) = self.scheduled_event() {
                if *scheduled_time <= cx.time() {
                    *self.scheduled_event() = None;
                }
            }
            let time = cx.time();
            let duration_since_prev_check = cx.time().duration_since(*self.previous_check_time());
            {
                let is_in_delay = self.delay_modes().active_delay().is_some();
                let is_in_process = self.process_state().is_some() && !is_in_delay;
                let is_env_blocked = matches!(self.env_state(), BasicEnvironmentState::Stopped);

                // Decrement process time counter (if not delayed or env blocked)
                if !(is_in_delay || is_env_blocked) {
                    if let Some((mut process_time_left, resource)) = self.process_state().take() {
                        process_time_left = process_time_left.saturating_sub(duration_since_prev_check);
                        if process_time_left.is_zero() {
                            *source_event_id = self.log(time, source_event_id.clone(), VectorProcessLogType::ProcessSuccess { quantity: resource.total(), vector: resource.clone() }).await;
                            self.push_downstream().send((resource.clone(), source_event_id.clone())).await;
                        } else {
                            *self.process_state() = Some((process_time_left, resource));
                        }
                    }
                }

                // Only case we don't update state here is if no delay is if we don't want the delay counters to decrement,
                // which is only the case if we're not processing and not in a delay - i.e. time-until-delay counters only decrement
                // when a process is active
                if !is_env_blocked && (is_in_delay || is_in_process) {
                    let delay_transition = self.delay_modes().update_state(duration_since_prev_check);
                    if delay_transition.has_changed() {
                        if let Some(delay_name) = &delay_transition.from {
                            *source_event_id = self.log(time, source_event_id.clone(), VectorProcessLogType::DelayEnd { delay_name: delay_name.clone() }).await;
                        }
                        if let Some(delay_name) = &delay_transition.to {
                            *source_event_id = self.log(time, source_event_id.clone(), VectorProcessLogType::DelayStart { delay_name: delay_name.clone() }).await;
                        }
                    }
                }
            }
        }
    }

    fn update_state_decision_logic(&mut self, source_event_id: &mut EventId, cx: &mut Context<Self>) -> impl Future<Output = ()> + Send {
        async move {
            let time = cx.time();
            {
                let new_env_state = match self.req_environment().send(()).await.next() {
                    Some(x) => x,
                    None => BasicEnvironmentState::Normal // Assume always normal operation if no environment state connected
                };
                match (&self.env_state(), &new_env_state) {
                    (BasicEnvironmentState::Normal, BasicEnvironmentState::Stopped) => {
                        *source_event_id = self.log(time, source_event_id.clone(), VectorProcessLogType::ProcessStopped { reason: "Stopped by environment" }).await;
                        *self.env_state() = BasicEnvironmentState::Stopped;
                    },
                    (BasicEnvironmentState::Stopped, BasicEnvironmentState::Normal) => {
                        *source_event_id = self.log(time, source_event_id.clone(), VectorProcessLogType::ProcessContinue { reason: "Resumed by environment" }).await;
                        *self.env_state() = BasicEnvironmentState::Normal;
                    }
                    _ => {}
                }
            }

            // Update internal state
            let is_env_stopped = matches!(self.env_state(), BasicEnvironmentState::Stopped);
            let has_active_delay = self.delay_modes().active_delay().is_some() || is_env_stopped;
            match (&self.process_state(), has_active_delay) {
                (None, false) => {
                    let us_state = self.req_upstream().send(()).await.next();
                    let ds_state = self.req_downstream().send(()).await.next();

                    println!("Checking upstream and downstream states: {:?} {:?}", us_state, ds_state);
                    match (&us_state, &ds_state) {
                        (
                            Some(VectorStockState::Normal {..}) | Some(VectorStockState::Full {..}),
                            Some(VectorStockState::Empty {..}) | Some(VectorStockState::Normal {..}),
                        ) => {
                            let process_quantity = self.process_quantity_distr().sample();
                            *source_event_id = self.log(time, source_event_id.clone(), VectorProcessLogType::WithdrawRequest).await;
                            let moved = self.withdraw_upstream().send((process_quantity, source_event_id.clone())).await.next().unwrap();
                            let process_duration_secs = self.process_time_distr().sample();
                            *self.process_state() = Some((Duration::from_secs_f64(process_duration_secs), moved.clone()));
                            *source_event_id = self.log(time, source_event_id.clone(), VectorProcessLogType::ProcessStart { quantity: process_quantity, vector: moved }).await;
                            *self.time_to_next_process_event() = Some(Duration::from_secs_f64(process_duration_secs));
                        },
                        (Some(VectorStockState::Empty {..} ), _) => {
                            *source_event_id = self.log(time, source_event_id.clone(), VectorProcessLogType::ProcessFailure { reason: "Upstream is empty" }).await;
                            *self.time_to_next_process_event() = None;
                        },
                        (None, _) => {
                            *source_event_id = self.log(time, source_event_id.clone(), VectorProcessLogType::ProcessFailure { reason: "Upstream is not connected" }).await;
                            *self.time_to_next_process_event() = None;
                        },
                        (_, None) => {
                            *source_event_id = self.log(time, source_event_id.clone(), VectorProcessLogType::ProcessFailure { reason: "Downstream is not connected" }).await;
                            *self.time_to_next_process_event() = None;
                        },
                        (_, Some(VectorStockState::Full {..} )) => {
                            *source_event_id = self.log(time, source_event_id.clone(), VectorProcessLogType::ProcessFailure { reason: "Downstream is full" }).await;
                            *self.time_to_next_process_event() = None;
                        },
                    }
                },
                (Some((time, _)), false) => {
                    *self.time_to_next_process_event() = Some(*time);
                },
                (_, true) => {
                    *self.time_to_next_process_event() = self.delay_modes().active_delay().map(|(_, delay_state)| *delay_state);
                }
            }
        }
    }

    fn update_state_next_event(&mut self, source_event_id: &mut EventId, cx: &mut Context<Self>) -> impl Future<Output = ()> + Send {
        async move {
            let is_env_stopped = matches!(self.env_state(), BasicEnvironmentState::Stopped);
            let has_active_delay = self.delay_modes().active_delay().is_some() || is_env_stopped;

            if self.process_state().is_some() || has_active_delay || !is_env_stopped {
                *self.time_to_next_delay_event() = self.delay_modes().get_next_event().map(|(_, delay_state)| delay_state.as_duration());
            } else {
                *self.time_to_next_delay_event() = None;
            }
            let time_to_next_event = [self.time_to_next_delay_event().clone(), self.time_to_next_process_event().clone()].into_iter().flatten().min();
            match time_to_next_event {
                None => {},
                Some(time_until_next) => {
                    if time_until_next.is_zero() {
                        panic!("Time until next event is zero!");
                    } else {
                        let next_time = cx.time() + time_until_next;

                        // Schedule event if sooner. If so, cancel previous event.
                        if let Some((scheduled_time, action_key)) = self.scheduled_event().take() {
                            if next_time < scheduled_time {
                                action_key.cancel();
                                let new_event_key =  cx.schedule_keyed_event(next_time, Self::update_state, source_event_id.clone()).unwrap();
                                *self.scheduled_event() = Some((next_time, new_event_key));
                            } else {
                                // Put the event back
                                *self.scheduled_event() = Some((scheduled_time, action_key));
                            }
                        } else {
                            let new_event_key =  cx.schedule_keyed_event(next_time, Self::update_state, source_event_id.clone()).unwrap();
                            *self.scheduled_event() = Some((next_time, new_event_key));
                        }
                    };
                }
            };
            *self.previous_check_time() = cx.time();
        }
    }

    fn log(
        &mut self, now: MonotonicTime, source_event_id: EventId, details: VectorProcessLogType<ResourceType>
    ) -> impl Future<Output = EventId> + Send {
        async move {
            let event_id = self.get_next_event_id();
            let log = self.to_record(source_event_id.clone(), event_id.clone(), details);
            self.log_emitter().send(log.clone()).await;
            event_id
        }
    }

    fn element_name(&self) -> &str;
    fn element_code(&self) -> &str;
    fn element_type(&self) -> &str;
    fn get_next_event_id(&mut self) -> EventId;
    fn log_emitter(&mut self) -> &mut Output<LogRecordType>;
    fn scheduled_event(&mut self) -> &mut Option<(MonotonicTime, ActionKey)>;
    fn previous_check_time(&mut self) -> &mut MonotonicTime;
    fn delay_modes(&mut self) -> &mut DelayModes;
    fn process_state(&mut self) -> &mut Option<(Duration, ResourceType)>;
    fn env_state(&mut self) -> &mut BasicEnvironmentState;
    fn req_environment(&mut self) -> &mut Requestor<(), BasicEnvironmentState>;
    fn req_upstream(&mut self) -> &mut Requestor<(), VectorStockState>;
    fn withdraw_upstream(&mut self) -> &mut Requestor<(f64, EventId), ResourceType>;
    fn req_downstream(&mut self) -> &mut Requestor<(), VectorStockState>;
    fn push_downstream(&mut self) -> &mut Output<(ResourceType, EventId)>;
    fn process_quantity_distr(&mut self) -> &mut Distribution;
    fn process_time_distr(&mut self) -> &mut Distribution;
    fn time_to_next_process_event(&mut self) -> &mut Option<Duration>;
    fn time_to_next_delay_event(&mut self) -> &mut Option<Duration>;

}


pub trait Projectable<T> where Self: ContinuousArithmetic {
    fn project(self, arg: T) -> Self;
}

#[derive(Debug, Clone, Serialize)]
pub struct VectorStockLog<T: ContinuousArithmetic> {
    pub time: String,
    pub event_id: EventId,
    pub source_event_id: EventId,
    pub element_name: String,
    pub element_type: String,
    pub details: VectorStockLogType<T>,
}

impl<T: ContinuousArithmetic + Debug + Serialize> VectorStockLog<T> {
    // type LogDetailsType = VectorStockLogType<T>;
    fn to_log(
        time: MonotonicTime,
        event_id: EventId,
        source_event_id: EventId,
        element_name: String,
        element_type: String,
        details: VectorStockLogType<T>,
    ) -> Self {
        VectorStockLog {
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
pub enum VectorStockLogType<T: ContinuousArithmetic> {
    Add { balance: f64, vector: T },
    Remove { balance: f64, vector: T },
    StateChange { new_state: VectorStockState },
}

pub trait StockState {
    fn is_same_state(&self, other: &Self) -> bool;
}
pub struct ExampleStockState {}
impl StockState for ExampleStockState {
    fn is_same_state(&self, other: &Self) -> bool {
        true
    }
}

#[derive(WithMethods)]
pub struct DefaultStock<T, S: StockState> where T: ContinuousArithmetic + Clone + Serialize + Send + 'static {
    // Identification
    pub element_name: String,
    pub element_code: String,
    pub element_type: String,

    // Ports
    pub log_emitter: Output<VectorStockLog<T>>,
    pub state_emitter: Output<EventId>,

    // Configuration
    pub low_capacity: f64,
    pub max_capacity: f64,

    // Runtime State
    pub resource: T,

    // Internals
    prev_state: Option<S>,
    next_event_id: u64,
}

impl<T: ContinuousResource + 'static, S: StockState + Send + 'static> Model for DefaultStock<T, S> {}

impl<T: ContinuousResource + Default + 'static, S: StockState> Default for DefaultStock<T, S> {
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
            next_event_id: 0,
        }
    }
}

impl<T: ContinuousResource + 'static> DefaultStock<T, VectorStockState> {

    fn get_state(&mut self) -> VectorStockState {
        let occupied = self.resource.total();
        let empty = self.max_capacity - occupied;
        if empty <= 0.0 {
            VectorStockState::Full { occupied, empty }
        } else if occupied < self.low_capacity {
            VectorStockState::Empty { occupied, empty }
        } else {
            VectorStockState::Normal { occupied, empty }
        }
    }

    pub fn get_state_async(&mut self) -> impl Future<Output = VectorStockState> {
        // TODO: Allow above to also have context arg?
        async move {
            let state = self.get_state();
            self.prev_state = Some(state.clone());
            state
        }
    }

    fn get_previous_state(&mut self) -> &Option<VectorStockState> {
        &self.prev_state
    }
    fn set_previous_state(&mut self) {
        self.prev_state = Some(self.get_state());
    }
    fn get_resource(&self) -> &T {
        &self.resource
    }

    pub fn add(&mut self, mut payload: (T, EventId), cx: &mut Context<Self>) -> impl Future<Output = ()> + where T: 'static {
        async move {
            // self.pre_add(&mut payload, cx).await;
            self.add_impl(&mut payload, cx).await;
            self.post_add(&mut payload, cx).await;
        }
    }

    fn add_impl(
        &mut self,
        payload: &mut (T, EventId),
        cx: &mut Context<Self>
    ) -> impl Future<Output=()> {
        async move {
            self.prev_state = Some(self.get_state().clone());
            self.resource.add(payload.0.clone());
            payload.1 = self.log(cx.time(), payload.1.clone(), VectorStockLogType::Add { balance: self.resource.total(), vector: payload.0.clone() }).await;
        }
    }

    fn post_add(&mut self, payload: &mut (T, EventId), cx: &mut Context<Self>) -> impl Future<Output = ()> + {
        async move {
            let previous_state = self.prev_state.clone();
            let current_state = self.get_state().clone();
            if previous_state.is_none() || !previous_state.as_ref().unwrap().is_same_state(&current_state) {
                // Send 1ns in future to avoid infinite loops with processes
                let next_time = cx.time() + Duration::from_nanos(1);
                cx.schedule_event(next_time, Self::emit_change, (current_state.clone(), payload.1.clone())).unwrap();
            }
            self.prev_state = Some(current_state);
        }
    }

    pub fn remove(&mut self, mut payload: (f64, EventId), cx: &mut Context<Self>) -> impl Future<Output = T> + where T: Projectable<f64> {
        async move {
            // self.pre_remove(&mut payload, cx).await;
            let result = self.remove_impl(&mut payload, cx).await;
            self.post_remove(&mut payload, cx).await;
            result
        }
    }

    fn remove_impl(
        &mut self,
        payload: &mut (f64, EventId),
        cx: &mut ::nexosim::model::Context<Self>
    ) -> impl Future<Output=T> where T: Projectable<f64> {
        async move {
            self.prev_state = Some(self.get_state());
            let result = self.resource.remove(payload.0);
            payload.1 = self.log(cx.time(), payload.1.clone(), VectorStockLogType::Remove { balance: self.resource.total(), vector: result.clone() }).await;
            result
        }
    }

    fn post_remove(&mut self, payload: &mut (f64, EventId), cx: &mut Context<Self>) -> impl Future<Output = ()> + {
          async move {
            let previous_state = self.prev_state.clone();
            let current_state = self.get_state().clone();
            match previous_state {
                None => {},
                Some(prev_state) => {
                    if !prev_state.is_same_state(&current_state) {
                        let next_time = cx.time() + Duration::from_nanos(1);
                        cx.schedule_event(next_time, Self::emit_change, (current_state.clone(), payload.1.clone())).unwrap();
                    }
                }
            }
            self.prev_state = Some(current_state);
        }
    }

    fn emit_change(&mut self, payload: (VectorStockState, EventId), cx: &mut nexosim::model::Context<Self>) -> impl Future<Output=()> {
        async move {
            let nm = self.log(cx.time(), payload.1, VectorStockLogType::StateChange { new_state: payload.0 }).await;
            self.state_emitter.send(nm).await;
        }
    }

    fn log<StockLogType: Into<VectorStockLogType<T>>>(&mut self, now: MonotonicTime, source_event_id: EventId, details: StockLogType) -> impl Future<Output = EventId> {
        async move {
            let new_event_id = EventId(format!("{}_{:06}", self.element_code, self.next_event_id));
            let log = VectorStockLog {
                time: now.to_chrono_date_time(0).unwrap().to_string(),
                event_id: new_event_id.clone(),
                source_event_id,
                element_name: self.element_name.clone(),
                element_type: self.element_type.clone(),
                details: details.into(),
            };
            self.log_emitter.send(log.clone()).await;
            self.next_event_id += 1;
            new_event_id
        }
    }
}

pub trait ContinuousArithmetic {
    fn add(&mut self, arg: Self);
    fn remove<T>(&mut self, arg: T) -> Self where Self: Projectable<T>;
    fn multiply(&mut self, arg: f64);
    fn total(&self) -> f64;
    fn remove_all(&mut self) -> Self;
}

pub trait ContinuousResource: ContinuousArithmetic + Clone + Send + Debug + Serialize {}
impl<T> ContinuousResource for T
where 
    T: ContinuousArithmetic + Clone + Send + Debug + Serialize {}

impl Projectable<f64> for f64 {
    fn project(self, arg: f64) -> f64 {
        arg
    }
}

impl<const N: usize> Projectable<f64> for [f64; N] {
    fn project(self, arg: f64) -> [f64; N] {
        let total = self.total();
        self.map(|x| x / total * arg)
    }
}

impl ContinuousArithmetic for f64 {
    fn add(&mut self, arg: Self) {
        *self += arg;
    }

    fn remove<T>(&mut self, arg: T) -> Self where Self: Projectable<T> {
        let removed = self.project(arg);
        *self -= removed;
        removed
    }

    fn multiply(&mut self, arg: f64) {
        *self *= arg;
    }

    fn total(&self) -> f64 {
        *self
    }

    fn remove_all(&mut self) -> Self {
        let removed = *self;
        *self = 0.0;
        removed
    }
}

impl<const N: usize> ContinuousArithmetic for [f64; N] {
    fn add(&mut self, arg: Self) {
        for (a, b) in self.iter_mut().zip(arg.iter()) {
            *a += *b;
        }
    }

    fn remove<T>(&mut self, arg: T) -> Self where Self: Projectable<T> {
        let removed = self.project(arg);
        for (a, b) in self.iter_mut().zip(removed.iter()) {
            *a -= *b;
        }
        removed
    }

    fn multiply(&mut self, arg: f64) {
        for a in self.iter_mut() {
            *a *= arg;
        }
    }

    fn total(&self) -> f64 {
        self.iter().sum()
    }

    fn remove_all(&mut self) -> Self {
        let removed = *self;
        for a in self.iter_mut() {
            *a = 0.0;
        }
        removed
    }
}

pub trait Connect<A: Model, B: Model> {
    fn connect(&mut self, a: (&mut A, &Address<A>), b: (&mut B, &Address<B>)) -> Result<(), String>;
}

pub struct Connection;

impl<
    T: ContinuousArithmetic + Clone + Send + Debug + Serialize + 'static,
> Connect<DefaultProcess<T, VectorProcessLog<T>>, DefaultStock<T, VectorStockState>> for Connection
    where DefaultProcess<T, VectorProcessLog<T>>: Model,
          DefaultStock<T, VectorStockState>: Model
{
    fn connect(
        &mut self,
        a: (&mut DefaultProcess<T, VectorProcessLog<T>>, &Address<DefaultProcess<T, VectorProcessLog<T>>>),
        b: (&mut DefaultStock<T, VectorStockState>, &Address<DefaultStock<T, VectorStockState>>),
    ) -> Result<(), String> {
        a.0.push_downstream.connect(DefaultStock::add, b.1.clone());
        a.0.req_downstream.connect(DefaultStock::get_state_async, b.1.clone());
        b.0.state_emitter.connect(DefaultProcess::update_state, a.1);
        Ok(())
    }
}

impl<
    T: ContinuousArithmetic + Clone + Send + Debug + Serialize + Projectable<f64> + 'static,
> Connect<DefaultStock<T, VectorStockState>, DefaultProcess<T, VectorProcessLog<T>>> for Connection
    where DefaultProcess<T, VectorProcessLog<T>>: Model,
          DefaultStock<T, VectorStockState>: Model
{
    fn connect(
        &mut self,
        a: (&mut DefaultStock<T, VectorStockState>, &Address<DefaultStock<T, VectorStockState>>),
        b: (&mut DefaultProcess<T, VectorProcessLog<T>>, &Address<DefaultProcess<T, VectorProcessLog<T>>>),
    ) -> Result<(), String> {
        b.0.withdraw_upstream.connect(DefaultStock::remove, a.1.clone());
        b.0.req_upstream.connect(DefaultStock::get_state_async, a.1.clone());
        a.0.state_emitter.connect(DefaultProcess::update_state, b.1);
        Ok(())
    }
}

impl<
    T: ContinuousArithmetic + Clone + Send + Debug + Serialize + Projectable<f64> + 'static,
> Connect<BasicEnvironment, DefaultProcess<T, VectorProcessLog<T>>> for Connection
    where DefaultStock<T, VectorStockState>: Model
{
    fn connect(
        &mut self,
        a: (&mut BasicEnvironment, &Address<BasicEnvironment>),
        b: (&mut DefaultProcess<T, VectorProcessLog<T>>, &Address<DefaultProcess<T, VectorProcessLog<T>>>),
    ) -> Result<(), String> {
        a.0.emit_change.connect(DefaultProcess::update_state, b.1.clone());
        b.0.req_environment.connect(BasicEnvironment::get_state_async, a.1.clone());
        Ok(())
    }
}