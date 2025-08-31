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

#[derive(WithMethods)]
pub struct DefaultStock<T, S: StockState>
where
    T: ContinuousArithmetic + Clone + Serialize + Send + 'static,
{
    // Identification
    pub element_name: String,
    pub element_code: String,
    pub element_type: String,

    // Ports
    pub log_emitter: Output<ContinuousStockLog<T>>,
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

impl<T: ContinuousResource + 'static> DefaultStock<T, ContinuousStockState> {
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

    pub fn get_state_async(&mut self) -> impl Future<Output = ContinuousStockState> {
        // TODO: Allow above to also have context arg?
        async move {
            let state = self.get_state();
            self.prev_state = Some(state.clone());
            state
        }
    }

    fn get_previous_state(&mut self) -> &Option<ContinuousStockState> {
        &self.prev_state
    }
    fn set_previous_state(&mut self) {
        self.prev_state = Some(self.get_state());
    }
    fn get_resource(&self) -> &T {
        &self.resource
    }

    pub fn add(
        &mut self,
        mut payload: (T, EventId),
        cx: &mut Context<Self>,
    ) -> impl Future<Output = ()>
    where
        T: 'static,
    {
        async move {
            // self.pre_add(&mut payload, cx).await;
            self.add_impl(&mut payload, cx).await;
            self.post_add(&mut payload, cx).await;
        }
    }

    fn add_impl(
        &mut self,
        payload: &mut (T, EventId),
        cx: &mut Context<Self>,
    ) -> impl Future<Output = ()> {
        async move {
            self.prev_state = Some(self.get_state().clone());
            self.resource.add(payload.0.clone());
            payload.1 = self
                .log(
                    cx.time(),
                    payload.1.clone(),
                    ContinuousStockLogType::Add {
                        balance: self.resource.total(),
                        resource: payload.0.clone(),
                    },
                )
                .await;
        }
    }

    fn post_add(
        &mut self,
        payload: &mut (T, EventId),
        cx: &mut Context<Self>,
    ) -> impl Future<Output = ()> {
        async move {
            let previous_state = self.prev_state.clone();
            let current_state = self.get_state().clone();
            if previous_state.is_none()
                || !previous_state
                    .as_ref()
                    .unwrap()
                    .is_same_state(&current_state)
            {
                // Send 1ns in future to avoid infinite loops with processes
                let next_time = cx.time() + Duration::from_nanos(1);
                cx.schedule_event(
                    next_time,
                    Self::emit_change,
                    (current_state.clone(), payload.1.clone()),
                )
                .unwrap();
            }
            self.prev_state = Some(current_state);
        }
    }

    pub fn remove(
        &mut self,
        mut payload: (f64, EventId),
        cx: &mut Context<Self>,
    ) -> impl Future<Output = T>
    where
        T: Projectable<f64>,
    {
        async move {
            let result = self.remove_impl(&mut payload, cx).await;
            self.post_remove(&mut payload, cx).await;
            result
        }
    }

    pub fn remove_void(
        &mut self,
        mut payload: (f64, EventId),
        cx: &mut Context<Self>,
    ) -> impl Future<Output = ()>
    where
        T: Projectable<f64>,
    {
        async move {
            self.remove(payload, cx).await;
        }
    }

    fn remove_impl(
        &mut self,
        payload: &mut (f64, EventId),
        cx: &mut ::nexosim::model::Context<Self>,
    ) -> impl Future<Output = T>
    where
        T: Projectable<f64>,
    {
        async move {
            self.prev_state = Some(self.get_state());
            let result = self.resource.remove(payload.0);
            payload.1 = self
                .log(
                    cx.time(),
                    payload.1.clone(),
                    ContinuousStockLogType::Remove {
                        balance: self.resource.total(),
                        resource: result.clone(),
                    },
                )
                .await;
            result
        }
    }

    fn post_remove(
        &mut self,
        payload: &mut (f64, EventId),
        cx: &mut Context<Self>,
    ) -> impl Future<Output = ()> {
        async move {
            let previous_state = self.prev_state.clone();
            let current_state = self.get_state().clone();
            match previous_state {
                None => {}
                Some(prev_state) => {
                    if !prev_state.is_same_state(&current_state) {
                        let next_time = cx.time() + Duration::from_nanos(1);
                        cx.schedule_event(
                            next_time,
                            Self::emit_change,
                            (current_state.clone(), payload.1.clone()),
                        )
                        .unwrap();
                    }
                }
            }
            self.prev_state = Some(current_state);
        }
    }

    fn emit_change(
        &mut self,
        payload: (ContinuousStockState, EventId),
        cx: &mut nexosim::model::Context<Self>,
    ) -> impl Future<Output = ()> {
        async move {
            let nm = self
                .log(
                    cx.time(),
                    payload.1,
                    ContinuousStockLogType::StateChange {
                        new_state: payload.0,
                    },
                )
                .await;
            self.state_emitter.send(nm).await;
        }
    }

    fn log<StockLogType: Into<ContinuousStockLogType<T>>>(
        &mut self,
        now: MonotonicTime,
        source_event_id: EventId,
        details: StockLogType,
    ) -> impl Future<Output = EventId> {
        async move {
            let new_event_id = EventId(format!("{}_{:06}", self.element_code, self.next_event_id));
            let log = ContinuousStockLog {
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
        source_event_id: EventId,
        event_id: EventId,
        details: DefaultProcessLogType<ResourceType>,
    ) -> ContinuousProcessLog<DefaultProcessLogType<ResourceType>, ResourceType> {
        ContinuousProcessLog::<DefaultProcessLogType<ResourceType>, ResourceType> {
            time: self
                .previous_check_time
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
    Process<T, ContinuousProcessLog<DefaultProcessLogType<T>, T>, DefaultProcessLogType<T>>
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
