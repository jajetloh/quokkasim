use futures::{future::join_all};
use nexosim::{model::Model, ports::{EventQueue, Output, Requestor}};
use serde::{ser::SerializeStruct, Serialize};
use tai_time::MonotonicTime;
use std::{collections::HashMap, fmt::Debug, time::Duration};

use crate::prelude::*;

/**
 * Stock
 */

#[derive(Debug, Clone)]
pub enum VectorStockState {
    Empty { occupied: f64, empty: f64 },
    Normal { occupied: f64, empty: f64 },
    Full { occupied: f64, empty: f64 },
}

impl VectorStockState {
    pub fn get_name(&self) -> String {
        match self {
            VectorStockState::Empty { .. } => "Empty".to_string(),
            VectorStockState::Normal { .. } => "Normal".to_string(),
            VectorStockState::Full { .. } => "Full".to_string(),
        }
    }
}

impl StateEq for VectorStockState {
    fn is_same_state(&self, other: &Self) -> bool {
        match (self, other) {
            (VectorStockState::Empty { .. }, VectorStockState::Empty { .. }) => true,
            (VectorStockState::Normal { .. }, VectorStockState::Normal { .. }) => true,
            (VectorStockState::Full { .. }, VectorStockState::Full { .. }) => true,
            _ => false,
        }
    }
}

pub struct VectorStock<T: Clone + Send + 'static> {
    pub element_name: String,
    pub element_code: String,
    pub element_type: String,
    pub vector: T,
    pub log_emitter: Output<VectorStockLog<T>>,
    pub state_emitter: Output<EventId>,
    pub low_capacity: f64,
    pub max_capacity: f64,
    pub prev_state: Option<VectorStockState>,
    next_event_id: u64,
}

impl<T: Clone + Default + Send> Default for VectorStock<T> {
    fn default() -> Self {
        VectorStock {
            element_name: String::new(),
            element_code: String::new(),
            element_type: String::new(),
            vector: Default::default(),
            low_capacity: 0.0,
            max_capacity: 0.0,
            log_emitter: Output::default(),
            state_emitter: Output::default(),
            prev_state: None,
            next_event_id: 0,
        }
    }
}

impl<T: Clone + Send> Stock<T, T, f64, T> for VectorStock<T>
where
    T: ResourceAdd<T> + ResourceRemove<f64, T> + ResourceTotal<f64>
{

    type StockState = VectorStockState;
    type LogDetailsType = VectorStockLogType<T>;

    fn get_state(&mut self) -> Self::StockState {
        let occupied = self.vector.total();
        let empty = self.max_capacity - occupied;
        if empty <= 0.0 {
            VectorStockState::Full { occupied, empty }
        } else if occupied < self.low_capacity {
            VectorStockState::Empty { occupied, empty }
        } else {
            VectorStockState::Normal { occupied, empty }
        }
    }

    fn get_previous_state(&mut self) -> &Option<Self::StockState> {
        &self.prev_state
    }
    fn set_previous_state(&mut self) {
        self.prev_state = Some(self.get_state());
    }
    fn get_resource(&self) -> &T {
        &self.vector
    }

    fn add_impl(
        &mut self,
        payload: &mut (T, EventId),
        cx: &mut ::nexosim::model::Context<Self>
    ) -> impl Future<Output=()> {
        async move {
            self.prev_state = Some(self.get_state().clone());
            self.vector.add(payload.0.clone());
            payload.1 = self.log(cx.time(), payload.1.clone(), VectorStockLogType::Add { quantity: payload.0.total(), vector: payload.0.clone() }).await;
        }
    }

    fn post_add(&mut self, payload: &mut (T, EventId), cx: &mut Context<Self>) -> impl Future<Output = ()> + {
        async move {
            let previous_state = self.prev_state.clone();
            let current_state = self.get_state().clone();
            if previous_state.is_none() || !previous_state.as_ref().unwrap().is_same_state(&current_state) {
                // Send 1ns in future to avoid infinite loops with processes
                let next_time = cx.time() + Duration::from_nanos(1);
                cx.schedule_event(next_time, Self::emit_change, payload.1.clone()).unwrap();
            }
            self.prev_state = Some(current_state);
        }
    }

    fn remove_impl(
        &mut self,
        payload: &mut (f64, EventId),
        cx: &mut ::nexosim::model::Context<Self>
    ) -> impl Future<Output=T> {
        async move {
            self.prev_state = Some(self.get_state());
            let result = self.vector.remove(payload.0);
            payload.1 = self.log(cx.time(), payload.1.clone(), VectorStockLogType::Remove { quantity: payload.0, vector: result.clone() }).await;
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
                        cx.schedule_event(next_time, Self::emit_change, payload.1.clone()).unwrap();
                    }
                }
            }
            self.prev_state = Some(current_state);
        }
    }

    fn emit_change(&mut self, source_event_id: EventId, cx: &mut nexosim::model::Context<Self>) -> impl Future<Output=()> {
        async move {
            let nm = self.log(cx.time(), source_event_id, VectorStockLogType::EmitChange).await;
            self.state_emitter.send(nm).await;
        }
    }

    fn log(&mut self, now: MonotonicTime, source_event_id: EventId, details: Self::LogDetailsType) -> impl Future<Output = EventId> {
        async move {
            let new_event_id = EventId(format!("{}_{:06}", self.element_code, self.next_event_id));
            let log = VectorStockLog {
                time: now.to_chrono_date_time(0).unwrap().to_string(),
                event_id: new_event_id.clone(),
                source_event_id,
                element_name: self.element_name.clone(),
                element_type: self.element_type.clone(),
                details,
            };
            self.log_emitter.send(log.clone()).await;
            self.next_event_id += 1;
            new_event_id
        }
    }
}

impl<T: Clone + Send> VectorStock<T>
where
    Self: Default,
    T: ResourceTotal<f64>
{
    pub fn get_state(&mut self) -> VectorStockState {
        let occupied = self.vector.total();
        let empty = self.max_capacity - occupied;
        if empty <= 0.0 {
            VectorStockState::Full { occupied, empty }
        } else if occupied < self.low_capacity {
            VectorStockState::Empty { occupied, empty }
        } else {
            VectorStockState::Normal { occupied, empty }
        }
    }

    pub fn with_name(self, name: String) -> Self {
        Self {
            element_name: name,
            ..self
        }
    }

    pub fn with_code(self, code: String) -> Self {
        Self {
            element_code: code,
            ..self
        }
    }

    pub fn with_type(self, element_type: String) -> Self {
        Self {
            element_type,
            ..self
        }
    }

    pub fn new() -> Self where T: Default {
        Self::default()
    }

    pub fn with_low_capacity(self, low_capacity: f64) -> Self {
        Self {
            low_capacity,
            ..self
        }
    }

    pub fn with_low_capacity_inplace(&mut self, low_capacity: f64) {
        self.low_capacity = low_capacity;
    }

    pub fn with_max_capacity(self, max_capacity: f64) -> Self {
        Self {
            max_capacity,
            ..self
        }
    }

    pub fn with_initial_vector(self, vector: T) -> Self {
        Self {
            vector,
            ..self
        }
    }
}

impl<T: Clone + Send> Model for VectorStock<T> {}

pub struct VectorStockLogger<T> where T: Send {
    pub name: String,
    pub buffer: EventQueue<VectorStockLog<T>>,
}

#[derive(Clone)]
pub struct VectorStockLog<T> {
    pub time: String,
    pub event_id: EventId,
    pub source_event_id: EventId,
    pub element_name: String,
    pub element_type: String,
    pub details: VectorStockLogType<T>,
}

#[derive(Clone)]
pub enum VectorStockLogType<T> {
    Add { quantity: f64, vector: T },
    Remove { quantity: f64, vector: T },
    EmitChange,
}

impl<T: Serialize> Serialize for VectorStockLog<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("VectorStockLog", 8)?;
        state.serialize_field("time", &self.time)?;
        state.serialize_field("event_id", &self.event_id)?;
        state.serialize_field("source_event_id", &self.source_event_id)?;
        state.serialize_field("element_name", &self.element_name)?;
        state.serialize_field("element_type", &self.element_type)?;
        let (log_type, total, vector_str): (&str, Option<f64>, Option<String>) = match &self.details {
            VectorStockLogType::Add { quantity, vector } => ("add", Some(*quantity), Some(serde_json::to_string(vector).map_err(serde::ser::Error::custom)?)),
            VectorStockLogType::Remove { quantity, vector } => ("remove", Some(*quantity), Some(serde_json::to_string(vector).map_err(serde::ser::Error::custom)?)),
            VectorStockLogType::EmitChange => ("emit_change", None, None),
        };
        state.serialize_field("log_type", &log_type)?;
        state.serialize_field("total", &total)?;
        state.serialize_field("vector", &vector_str)?;
        // state.serialize_field("log_type", &self.message)?;
        // state.serialize_field("state", &self.state.get_name())?;
        // let vector_json = serde_json::to_string(&self.vector).map_err(serde::ser::Error::custom)?;
        // state.serialize_field("vector", &vector_json)?;
        state.end()
    }
}

impl<T: Serialize + Send + 'static> Logger for VectorStockLogger<T> {
    type RecordType = VectorStockLog<T>;
    fn get_name(&self) -> &String {
        &self.name
    }
    fn get_buffer(self) -> EventQueue<Self::RecordType> {
        self.buffer
    }
    fn new(name: String) -> Self {
        VectorStockLogger {
            name,
            buffer: EventQueue::new(),
        }
    }
}

/**
 * Process
 */

pub struct VectorProcess<
    ReceiveParameterType: Clone + Send + 'static,
    ReceiveType: Clone + Send + 'static,
    InternalResourceType: Clone + Send + 'static,
    SendType: Clone + Send + 'static,
> {
    pub element_name: String,
    pub element_code: String,
    pub element_type: String,
    pub req_upstream: Requestor<(), VectorStockState>,
    pub req_downstream: Requestor<(), VectorStockState>,
    pub withdraw_upstream: Requestor<(ReceiveParameterType, EventId), ReceiveType>,
    pub push_downstream: Output<(SendType, EventId)>,
    pub process_state: Option<(Duration, InternalResourceType)>,
    pub process_quantity_distr: Distribution,
    pub process_time_distr: Distribution,
    pub delay_modes: DelayModes,
    time_to_next_event: Option<Duration>,
    scheduled_event: Option<(MonotonicTime, ActionKey)>,
    next_event_index: u64,
    pub log_emitter: Output<VectorProcessLog<InternalResourceType>>,
    pub previous_check_time: MonotonicTime,
}
impl<
    ReceiveParameterType: Clone + Send,
    ReceiveType: Clone + Send,
    InternalResourceType: Clone + Send,
    SendType: Clone + Send
> Default for VectorProcess<ReceiveParameterType, ReceiveType, InternalResourceType, SendType> {
    fn default() -> Self {
        VectorProcess {
            element_name: String::new(),
            element_code: String::new(),
            element_type: String::new(),
            process_quantity_distr: Distribution::default(),
            process_time_distr: Distribution::default(),
            delay_modes: DelayModes::default(),

            req_upstream: Requestor::default(),
            req_downstream: Requestor::default(),
            withdraw_upstream: Requestor::default(),
            push_downstream: Output::default(),
            log_emitter: Output::default(),
            
            process_state: None,
            time_to_next_event: None,
            scheduled_event: None,
            next_event_index: 0,
            previous_check_time: MonotonicTime::EPOCH,
        }
    }
}

impl<
    ReceiveParameterType: Clone + Send,
    ReceiveType: Clone + Send,
    InternalResourceType: Clone + Send,
    SendType: Clone + Send
> Model for VectorProcess<ReceiveParameterType, ReceiveType, InternalResourceType, SendType> where Self: Process {
    fn init(mut self, ctx: &mut Context<Self>) -> impl Future<Output = InitializedModel<Self>> + Send {
        async move {
            let source_event_id = EventId::from_init();
            self.update_state(source_event_id, ctx).await;
            self.into()
        }
    }
}

impl<T: Clone + Send> Process for VectorProcess<f64, T, T, T>
where
    T: ResourceAdd<T> + ResourceRemove<f64, T> + ResourceTotal<f64>,
{
    type LogDetailsType = VectorProcessLogType<T>;
    
    fn pre_update_state(&mut self, source_event_id: &mut EventId, cx: &mut Context<Self>) -> impl Future<Output = ()> {
        async move {
            if let Some((scheduled_time, _)) = self.scheduled_event.as_ref() {
                if *scheduled_time <= cx.time() {
                    self.scheduled_event = None;
                }
            }
        }
    }
    fn update_state_impl(&mut self, source_event_id: &mut EventId, cx: &mut nexosim::model::Context<Self>) -> impl Future<Output = ()> {
        async move {
            let time = cx.time();
            let duration_since_prev_check = cx.time().duration_since(self.previous_check_time);

            // Update duration counters based on time since last check
            {
                let is_in_delay = self.delay_modes.active_delay().is_some();
                let is_in_process = self.process_state.is_some() && !is_in_delay;

                if !is_in_delay {
                    if let Some((mut process_time_left, resource)) = self.process_state.take() {
                        process_time_left = process_time_left.saturating_sub(duration_since_prev_check);
                        if process_time_left.is_zero() {
                            *source_event_id = self.log(time, source_event_id.clone(), VectorProcessLogType::ProcessSuccess { quantity: resource.total(), vector: resource.clone() }).await;
                            self.push_downstream.send((resource.clone(), source_event_id.clone())).await;
                        } else {
                            self.process_state = Some((process_time_left, resource));
                        }
                    }
                }

                // Only case we don't update state here is if no delay is if we don't want the delay counters to decrement,
                // which is only the case if we're not processing and not in a delay - i.e. time-until-delay counters only decrement
                // when a process is active
                if is_in_delay || is_in_process {
                    let delay_transition = self.delay_modes.update_state(duration_since_prev_check.clone());
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

            // Update internal state
            let has_active_delay = self.delay_modes.active_delay().is_some();
            match (&self.process_state, has_active_delay) {
                (None, false) => {
                    let us_state = self.req_upstream.send(()).await.next();
                    let ds_state = self.req_downstream.send(()).await.next();
                    match (&us_state, &ds_state) {
                        (
                            Some(VectorStockState::Normal {..}) | Some(VectorStockState::Full {..}),
                            Some(VectorStockState::Empty {..}) | Some(VectorStockState::Normal {..}),
                        ) => {
                            let process_quantity = self.process_quantity_distr.sample();
                            *source_event_id = self.log(time, source_event_id.clone(), VectorProcessLogType::WithdrawRequest).await;
                            let moved = self.withdraw_upstream.send((process_quantity, source_event_id.clone())).await.next().unwrap();
                            let process_duration_secs = self.process_time_distr.sample();
                            self.process_state = Some((Duration::from_secs_f64(process_duration_secs), moved.clone()));
                            *source_event_id = self.log(time, source_event_id.clone(), VectorProcessLogType::ProcessStart { quantity: process_quantity, vector: moved }).await;
                            self.time_to_next_event = Some(Duration::from_secs_f64(process_duration_secs));
                        },
                        (Some(VectorStockState::Empty {..} ), _) => {
                            *source_event_id = self.log(time, source_event_id.clone(), VectorProcessLogType::ProcessFailure { reason: "Upstream is empty" }).await;
                            self.time_to_next_event = None;
                        },
                        (None, _) => {
                            *source_event_id = self.log(time, source_event_id.clone(), VectorProcessLogType::ProcessFailure { reason: "Upstream is not connected" }).await;
                            self.time_to_next_event = None;
                        },
                        (_, None) => {
                            *source_event_id = self.log(time, source_event_id.clone(), VectorProcessLogType::ProcessFailure { reason: "Downstream is not connected" }).await;
                            self.time_to_next_event = None;
                        },
                        (_, Some(VectorStockState::Full {..} )) => {
                            *source_event_id = self.log(time, source_event_id.clone(), VectorProcessLogType::ProcessFailure { reason: "Downstream is full" }).await;
                            self.time_to_next_event = None;
                        },
                    }
                },
                (Some((time, _)), false) => {
                    self.time_to_next_event = Some(*time);
                },
                (_, true) => {
                    self.time_to_next_event = Some(self.delay_modes.active_delay().unwrap().1.clone())
                }
            }
        }
    }

    fn post_update_state(&mut self, source_event_id: &mut EventId, cx: &mut nexosim::model::Context<Self>) -> impl Future<Output = ()> + Send {
        async move {
            match self.time_to_next_event {
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

    fn log(&mut self, now: MonotonicTime, source_event_id: EventId, details: Self::LogDetailsType) -> impl Future<Output = EventId> {
        async move {
            let new_event_id = EventId(format!("{}_{:06}", self.element_code, self.next_event_index));
            let log = VectorProcessLog {
                time: now.to_chrono_date_time(0).unwrap().to_string(),
                event_id: new_event_id.clone(),
                source_event_id,
                element_name: self.element_name.clone(),
                element_type: self.element_type.clone(),
                event: details,
            };
            self.log_emitter.send(log.clone()).await;
            self.next_event_index += 1;

            new_event_id
        }
    }
}

impl<T: Clone + Send> VectorProcess<f64, T, T, T> {
    pub fn with_name(self, name: String) -> Self {
        VectorProcess {
            element_name: name,
            ..self
        }
    }

    pub fn with_code(self, code: String) -> Self {
        Self {
            element_code: code,
            ..self
        }
    }

    pub fn with_type(self, element_type: String) -> Self {
        Self {
            element_type,
            ..self
        }
    }

    pub fn new() -> Self where T: Default {
        Self::default()
    }

    pub fn with_process_quantity_distr(self, process_quantity_distr: Distribution) -> Self {
        Self {
            process_quantity_distr,
            ..self
        }
    }

    pub fn with_process_quantity_distr_inplace(&mut self, process_quantity_distr: Distribution) {
        self.process_quantity_distr = process_quantity_distr;
    }

    pub fn with_process_time_distr(self, process_time_distr: Distribution) -> Self {
        VectorProcess {
            process_time_distr,
            ..self
        }
    }

    /// Adds a delay if provided, or attempts to remove it if `None`
    pub fn with_delay(mut self, delay_mode_change: DelayModeChange) -> Self {
        self.delay_modes.modify(delay_mode_change);
        self
    }

    /// Adds a delay if provided, or attempts to remove it if `None`
    pub fn with_delay_inplace(&mut self, delay_mode_change: DelayModeChange) {
        self.delay_modes.modify(delay_mode_change);
    }
} 

pub struct VectorProcessLogger<T> where T: Send {
    pub name: String,
    pub buffer: EventQueue<VectorProcessLog<T>>,
}

impl<T> Logger for VectorProcessLogger<T> where VectorProcessLog<T>: Serialize, T: Send + 'static {
    type RecordType = VectorProcessLog<T>;
    fn get_name(&self) -> &String {
        &self.name
    }
    fn get_buffer(self) -> EventQueue<Self::RecordType> {
        self.buffer
    }
    fn new(name: String) -> Self {
        VectorProcessLogger {
            name,
            buffer: EventQueue::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum VectorProcessLogType<T> {
    ProcessStart { quantity: f64, vector: T },
    ProcessSuccess { quantity: f64, vector: T },
    ProcessFailure { reason: &'static str },
    CombineStart { quantity: f64, vectors: Vec<T> },
    CombineSuccess { quantity: f64, vector: T},
    CombineFailure { reason: &'static str },
    SplitStart { quantity: f64, vector: T },
    SplitSuccess { quantity: f64, vectors: Vec<T> },
    SplitFailure { reason: &'static str },
    WithdrawRequest,
    PushRequest,
    DelayStart { delay_name: String },
    DelayEnd { delay_name: String },
}

#[derive(Debug, Clone)]
pub struct VectorProcessLog<T> {
    pub time: String,
    pub event_id: EventId,
    pub source_event_id: EventId,
    pub element_name: String,
    pub element_type: String,
    pub event: VectorProcessLogType<T>,
}

impl<T> Serialize for VectorProcessLog<T> where T: Serialize + Send {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("VectorProcessLog", 10)?;
        state.serialize_field("time", &self.time)?;
        state.serialize_field("event_id", &self.event_id)?;
        state.serialize_field("source_event_id", &self.source_event_id)?;
        state.serialize_field("element_name", &self.element_name)?;
        state.serialize_field("element_type", &self.element_type)?;
        let (event_type, total, inflows, outflows, reason): (&str, Option<f64>, Option<String>, Option<String>, Option<String>);
        match &self.event {
            VectorProcessLogType::ProcessStart { quantity, vector } => {
                event_type = "ProcessStart";
                total = Some(*quantity);
                inflows = Some(serde_json::to_string(&vec![vector]).map_err(|e| serde::ser::Error::custom(e.to_string()))?);
                outflows = None;
                reason = None;
            },
            VectorProcessLogType::ProcessSuccess { quantity, vector } => {
                event_type = "ProcessSuccess";
                total = Some(*quantity);
                inflows = None;
                outflows = Some(serde_json::to_string(&vec![vector]).map_err(|e| serde::ser::Error::custom(e.to_string()))?);
                reason = None;
            },
            VectorProcessLogType::ProcessFailure { reason: r } => {
                event_type = "ProcessFailure";
                total = None;
                inflows = None;
                outflows = None;
                reason = Some(r.to_string());
            },
            VectorProcessLogType::CombineStart { quantity, vectors } => {
                event_type = "CombineStart";
                total = Some(*quantity);
                inflows = Some(serde_json::to_string(vectors).map_err(|e| serde::ser::Error::custom(e.to_string()))?);
                outflows = None;
                reason = None;
            },
            VectorProcessLogType::CombineSuccess { quantity, vector } => {
                event_type = "CombineSuccess";
                total = Some(*quantity);
                inflows = None;
                outflows = Some(serde_json::to_string(&vec![vector]).map_err(|e| serde::ser::Error::custom(e.to_string()))?);
                reason = None;
            },
            VectorProcessLogType::CombineFailure { reason: r } => {
                event_type = "CombineFailure";
                total = None;
                inflows = None;
                outflows = None;
                reason = Some(r.to_string());
            },
            VectorProcessLogType::SplitStart { quantity, vector } => {
                event_type = "SplitStart";
                total = Some(*quantity);
                inflows = Some(serde_json::to_string(&vec![vector]).map_err(|e| serde::ser::Error::custom(e.to_string()))?);
                outflows = None;
                reason = None;
            },
            VectorProcessLogType::SplitSuccess { quantity, vectors } => {
                event_type = "SplitSuccess";
                total = Some(*quantity);
                inflows = None;
                outflows = Some(serde_json::to_string(vectors).map_err(|e| serde::ser::Error::custom(e.to_string()))?);
                reason = None;
            },
            VectorProcessLogType::SplitFailure { reason: r } => {
                event_type = "SplitFailure";
                total = None;
                inflows = None;
                outflows = None;
                reason = Some(r.to_string());
            },
            VectorProcessLogType::WithdrawRequest => {
                event_type = "WithdrawRequest";
                total = None;
                inflows = None;
                outflows = None;
                reason = None;
            },
            VectorProcessLogType::PushRequest => {
                event_type = "PushRequest";
                total = None;
                inflows = None;
                outflows = None;
                reason = None;
            },
            VectorProcessLogType::DelayStart { delay_name } => {
                event_type = "DelayStart";
                total = None;
                inflows = None;
                outflows = None;
                reason = Some(delay_name.clone());
            },
            VectorProcessLogType::DelayEnd { delay_name } => {
                event_type = "DelayEnd";
                total = None;
                inflows = None;
                outflows = None;
                reason = Some(delay_name.clone());
            },
        }
        state.serialize_field("event_type", &event_type)?;
        state.serialize_field("total", &total)?;
        state.serialize_field("inflows", &inflows)?;
        state.serialize_field("outflows", &outflows)?;
        state.serialize_field("reason", &reason)?;
        state.end()
    }
}

/**
 * Combiner
 */

 pub struct VectorCombiner<
    ReceiveParameterType: Clone + Send + 'static,
    ReceiveType: Clone + Send + 'static,
    InternalResourceType: Clone + Send + 'static,
    SendType: Clone + Send + 'static,
    const M: usize
> {
    pub element_name: String,
    pub element_code: String,
    pub element_type: String,
    pub req_upstreams: [Requestor<(), VectorStockState>; M],
    pub req_downstream: Requestor<(), VectorStockState>,
    pub withdraw_upstreams: [Requestor<(ReceiveParameterType, EventId), ReceiveType>; M],
    pub push_downstream: Output<(SendType, EventId)>,
    pub process_state: Option<(Duration, InternalResourceType)>,
    pub process_quantity_distr: Distribution,
    pub process_time_distr: Distribution,
    time_to_next_event: Option<Duration>,
    scheduled_event: Option<(MonotonicTime, ActionKey)>,
    next_event_index: u64,
    pub log_emitter: Output<VectorProcessLog<ReceiveType>>,
    pub previous_check_time: MonotonicTime,
    pub split_ratios: [f64; M],
}

impl<
    ReceiveParameterType: Clone + Send + 'static,
    ReceiveType: Clone + Send + 'static,
    InternalResourceType: Clone + Send + 'static,
    SendType: Clone + Send + 'static,
    const M: usize
> Model for VectorCombiner<ReceiveParameterType, ReceiveType, InternalResourceType, SendType, M> where Self: Process {
    fn init(mut self, ctx: &mut Context<Self>) -> impl Future<Output = InitializedModel<Self>> + Send {
        async move {
            let source_event_id = EventId(format!("{}_{:06}", self.element_code, self.next_event_index));
            self.update_state(source_event_id, ctx).await;
            self.into()
        }
    }
}

impl<
    T: Clone + Send + 'static,
    U: Clone + Send,
    const M: usize
> Default for VectorCombiner<U, T, [T; M], T, M> {
    fn default() -> Self {
        Self::new()
    }
}

impl<
    T: Clone + Send + 'static,
    U: Clone + Send,
    const M: usize
> VectorCombiner<U, T, [T; M], T, M> {
    pub fn new() -> Self {
        VectorCombiner {
            element_name: String::new(),
            element_code: String::new(),
            element_type: String::new(),
            req_upstreams: std::array::from_fn(|_| Requestor::default()),
            req_downstream: Requestor::default(),
            withdraw_upstreams: std::array::from_fn(|_| Requestor::default()),
            push_downstream: Output::default(),
            process_state: None,
            process_quantity_distr: Distribution::default(),
            process_time_distr: Distribution::default(),
            time_to_next_event: None,
            scheduled_event: None,
            next_event_index: 0,
            log_emitter: Output::default(),
            previous_check_time: MonotonicTime::EPOCH,
            split_ratios: [1./(M as f64); M],
        }
    }

    pub fn with_name(self, name: String) -> Self {
        Self {
            element_name: name,
            ..self
        }
    }

    pub fn with_code(self, code: String) -> Self {
        Self {
            element_code: code,
            ..self
        }
    }

    pub fn with_process_quantity_distr(self, process_quantity_distr: Distribution) -> Self {
        Self {
            process_quantity_distr,
            ..self
        }
    }

    pub fn with_process_time_distr(self, process_time_distr: Distribution) -> Self {
        Self {
            process_time_distr,
            ..self
        }
    }
}

impl<T: Send + 'static + Clone + Default, const M: usize> Process for VectorCombiner<f64, T, [T; M], T, M>
where
    T: ResourceAdd<T> + ResourceTotal<f64>,
{
    type LogDetailsType = VectorProcessLogType<T>;

    fn pre_update_state(&mut self, source_event_id: &mut EventId, cx: &mut Context<Self>) -> impl Future<Output = ()> + Send where Self: Model {
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

            if let Some((mut process_time_left, resources)) = self.process_state.take() {
                let duration_since_prev_check = cx.time().duration_since(self.previous_check_time);
                process_time_left = process_time_left.saturating_sub(duration_since_prev_check);
                if process_time_left.is_zero() {
                    let mut total: T = Default::default();

                    for resource in resources.iter() {
                        total.add(resource.clone());
                    }

                    *source_event_id = self.log(time, source_event_id.clone(), VectorProcessLogType::CombineSuccess { quantity: resources.iter().map(|x| x.total()).sum(), vector: total.clone() }).await;
                    self.push_downstream.send((total, source_event_id.clone())).await;
                } else {
                    self.process_state = Some((process_time_left, resources));
                }
            }
            match self.process_state {
                None => {
                    let iterators = join_all(self.req_upstreams.iter_mut().map(|req| {
                        req.send(())
                    })).await;
                    let us_states: Vec<VectorStockState> = iterators.into_iter().flatten().collect();
                    let all_us_available: Option<bool>;
                    if us_states.len() < M {
                        all_us_available = None;
                    } else {
                        all_us_available = Some(us_states.iter().all(|state| {
                            matches!(state, VectorStockState::Normal {..} | VectorStockState::Full {..})
                        }));
                    }
                    let ds_state = self.req_downstream.send(()).await.next();
                    match (all_us_available, ds_state) {
                        (
                            Some(true),
                            Some(VectorStockState::Empty {..}) | Some(VectorStockState::Normal {..}),
                        ) => {
                            let process_quantity = self.process_quantity_distr.sample();
                            *source_event_id = self.log(time, source_event_id.clone(), VectorProcessLogType::WithdrawRequest).await;
                            let withdraw_iterators = join_all(self.withdraw_upstreams.iter_mut().map(|req| {
                                req.send((process_quantity, source_event_id.clone()))
                            })).await;
                            let withdrawn: [T; M] = withdraw_iterators.into_iter()
                                .map(|mut x| x.next().unwrap_or_else(|| Default::default()))
                                .collect::<Vec<T>>()
                                .try_into()
                                .unwrap_or_else(|_| panic!("Failed to convert to array"));
                            let process_duration_secs = self.process_time_distr.sample();
                            self.process_state = Some((Duration::from_secs_f64(process_duration_secs), withdrawn.clone()));
                            *source_event_id = self.log(time, source_event_id.clone(), VectorProcessLogType::CombineStart { quantity: process_quantity, vectors: withdrawn.into() }).await;
                            self.time_to_next_event = Some(Duration::from_secs_f64(process_duration_secs));
                        },
                        (Some(false), _) => {
                            *source_event_id = self.log(time, source_event_id.clone(), VectorProcessLogType::ProcessFailure { reason: "At least one upstream is empty" }).await;
                            self.time_to_next_event = None;
                        },
                        (None, _) => {
                            *source_event_id = self.log(time, source_event_id.clone(), VectorProcessLogType::ProcessFailure { reason: "No upstreams are connected" }).await;
                            self.time_to_next_event = None;
                        },
                        (_, None) => {
                            *source_event_id = self.log(time, source_event_id.clone(), VectorProcessLogType::ProcessFailure { reason: "Downstream is not connected" }).await;
                            self.time_to_next_event = None;
                        },
                        (_, Some(VectorStockState::Full {..} )) => {
                            *source_event_id = self.log(time, source_event_id.clone(), VectorProcessLogType::ProcessFailure { reason: "Downstream is full" }).await;
                            self.time_to_next_event = None;
                        },
                    }
                },
                Some((time, _)) => {
                    self.time_to_next_event = Some(time);
                }
            }
        }
    }

    fn post_update_state(&mut self, source_event_id: &mut EventId, cx: &mut nexosim::model::Context<Self>) -> impl Future<Output = ()> + Send {
        async move {
            match self.time_to_next_event {
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

    fn log(&mut self, now: MonotonicTime, source_event_id: EventId, details: Self::LogDetailsType) -> impl Future<Output = EventId> {
        async move {
            let new_event_id = EventId(format!("{}_{:06}", self.element_code, self.next_event_index));
            let log = VectorProcessLog {
                time: now.to_chrono_date_time(0).unwrap().to_string(),
                event_id: new_event_id.clone(),
                source_event_id,
                element_name: self.element_name.clone(),
                element_type: self.element_type.clone(),
                event: details,
            };
            self.log_emitter.send(log.clone()).await;
            self.next_event_index += 1;

            new_event_id
        }
    }
}


/**
 * Splitter
 */

pub struct VectorSplitter<
    ReceiveParameterType: Clone + Send + 'static,
    ReceiveType: Clone + Send + 'static,
    InternalResourceType: Clone + Send + 'static,
    SendType: Clone + Send + 'static,
    const N: usize
> {
    pub element_name: String,
    pub element_code: String,
    pub element_type: String,
    pub req_upstream: Requestor<(), VectorStockState>,
    pub req_downstreams: [Requestor<(), VectorStockState>; N],
    pub withdraw_upstream: Requestor<(ReceiveParameterType, EventId), ReceiveType>,
    pub push_downstreams: [Output<(SendType, EventId)>; N],
    pub process_state: Option<(Duration, InternalResourceType)>,
    pub process_quantity_distr: Distribution,
    pub process_time_distr: Distribution,
    time_to_next_event: Option<Duration>,
    scheduled_event: Option<(MonotonicTime, ActionKey)>,
    next_event_index: u64,
    pub log_emitter: Output<VectorProcessLog<ReceiveType>>,
    pub previous_check_time: MonotonicTime,
    pub split_ratios: [f64; N],
}

impl<T: Send + 'static + Clone + Default, const N: usize> VectorSplitter<f64, T, T, T, N> where Self: Default {
    pub fn new() -> Self {
        Default::default()
    }
    
    pub fn with_name(self, name: String) -> Self {
        Self {
            element_name: name,
            ..self
        }
    }

    pub fn with_code(self, code: String) -> Self {
        Self {
            element_code: code,
            ..self
        }
    }

    pub fn with_process_quantity_distr(self, process_quantity_distr: Distribution) -> Self {
        Self {
            process_quantity_distr,
            ..self
        }
    }

    pub fn with_process_time_distr(self, process_time_distr: Distribution) -> Self {
        Self {
            process_time_distr,
            ..self
        }
    }
}

impl<
    ReceiveParameterType: Clone + Send + 'static,
    ReceiveType: Clone + Send + 'static,
    InternalResourceType: Clone + Send + 'static,
    SendType: Clone + Send + 'static,
    const N: usize
> Default for VectorSplitter<ReceiveParameterType, ReceiveType, InternalResourceType, SendType, N> {
    fn default() -> Self {
        VectorSplitter {
            element_name: String::new(),
            element_code: String::new(),
            element_type: String::new(),
            req_upstream: Requestor::default(),
            req_downstreams: std::array::from_fn(|_| Requestor::default()),
            withdraw_upstream: Requestor::default(),
            push_downstreams: std::array::from_fn(|_| Output::default()),
            process_state: None,
            process_quantity_distr: Distribution::default(),
            process_time_distr: Distribution::default(),
            time_to_next_event: None,
            scheduled_event: None,
            next_event_index: 0,
            log_emitter: Output::default(),
            previous_check_time: MonotonicTime::EPOCH,
            split_ratios: [1./(N as f64); N],
        }
    }
}

impl<
    ReceiveParameterType: Clone + Send + 'static,
    ReceiveType: Clone + Send + 'static,
    InternalResourceType: Clone + Send + 'static,
    SendType: Clone + Send + 'static,
    const N: usize
> Model for VectorSplitter<ReceiveParameterType, ReceiveType, InternalResourceType, SendType, N> where Self: Process {
    fn init(mut self, ctx: &mut Context<Self>) -> impl Future<Output = InitializedModel<Self>> + Send {
        async move {
            let source_event_id = EventId(format!("{}_{:06}", self.element_code, self.next_event_index));
            self.update_state(source_event_id, ctx).await;
            self.into()
        }
    }
}

impl<T: Send + 'static + Clone + Default, const N: usize> Process for VectorSplitter<f64, T, T, T, N>
where
    T: ResourceRemove<f64, T> + ResourceTotal<f64>,
{
    type LogDetailsType = VectorProcessLogType<T>;

    fn pre_update_state(&mut self, source_event_id: &mut EventId, cx: &mut Context<Self>) -> impl Future<Output = ()> {
        async move {
            if let Some((scheduled_time, _)) = self.scheduled_event.as_ref() {
                if *scheduled_time <= cx.time() {
                    self.scheduled_event = None;
                }
            }
        }
    }

    fn update_state_impl(&mut self, source_event_id: &mut EventId, cx: &mut nexosim::model::Context<Self>) -> impl Future<Output = ()> {
        async move {
            let time = cx.time();

            if let Some((mut process_time_left, resource)) = self.process_state.take() {
                            let duration_since_prev_check = cx.time().duration_since(self.previous_check_time);
                            process_time_left = process_time_left.saturating_sub(duration_since_prev_check);

                            if process_time_left.is_zero() {
            
                                let split_resources = self.split_ratios.iter().map(|ratio| {
                                    let quantity = resource.total() * ratio;
                                    resource.clone().remove(quantity)
                                }).collect::<Vec<_>>();

                                *source_event_id = self.log(time, source_event_id.clone(), VectorProcessLogType::SplitSuccess { quantity: resource.total(), vectors: split_resources.clone() }).await;

                                join_all(self.push_downstreams.iter_mut().zip(split_resources).map(|(push, resource)| {
                                    push.send((resource.clone(), source_event_id.clone()))
                                })).await;
                            } else {
                                self.process_state = Some((process_time_left, resource));
                            }
                        }
            match self.process_state {
                None => {
                    let us_state = self.req_upstream.send(()).await.next();
                    let ds_states = join_all(self.req_downstreams.iter_mut().map(|req| req.send(()))).await.iter_mut().map(|x| {
                        x.next()
                    }).collect::<Vec<Option<VectorStockState>>>();
                    let all_ds_available: Option<bool>;
                    if ds_states.len() < N {
                        all_ds_available = None;
                    } else {
                        all_ds_available = Some(ds_states.iter().all(|state| {
                            matches!(state, Some(VectorStockState::Normal {..}) | Some(VectorStockState::Empty {..}))
                        }));
                    }

                    match (us_state, all_ds_available) {
                        (
                            Some(VectorStockState::Full {..}) | Some(VectorStockState::Normal {..}),
                            Some(true),
                        ) => {
                            let process_quantity = self.process_quantity_distr.sample();
                            *source_event_id = self.log(time, source_event_id.clone(), VectorProcessLogType::WithdrawRequest).await;
                            let withdrawn = self.withdraw_upstream.send((process_quantity, source_event_id.clone())).await.next().unwrap();
                            let process_duration_secs = self.process_time_distr.sample();
                            self.process_state = Some((Duration::from_secs_f64(process_duration_secs), withdrawn.clone()));
                            *source_event_id = self.log(time, source_event_id.clone(), VectorProcessLogType::SplitStart { quantity: process_quantity, vector: withdrawn }).await;
                            self.time_to_next_event = Some(Duration::from_secs_f64(process_duration_secs));
                        },
                        (_, Some(false)) => {
                            *source_event_id = self.log(time, source_event_id.clone(), VectorProcessLogType::ProcessFailure { reason: "At least one downstream is full" }).await;
                            self.time_to_next_event = None;
                        },
                        (_, None) => {
                            *source_event_id = self.log(time, source_event_id.clone(), VectorProcessLogType::ProcessFailure { reason: "No downstreams are connected" }).await;
                            self.time_to_next_event = None;
                        },
                        (None, _) => {
                            *source_event_id = self.log(time, source_event_id.clone(), VectorProcessLogType::ProcessFailure { reason: "Upstream is not connected" }).await;
                            self.time_to_next_event = None;
                        },
                        (Some(VectorStockState::Empty {..} ), _) => {
                            *source_event_id = self.log(time, source_event_id.clone(), VectorProcessLogType::ProcessFailure { reason: "Upstream is empty" }).await;
                            self.time_to_next_event = None;
                        },
                    }
                },
                Some((time, _)) => {
                    self.time_to_next_event = Some(time);
                }
            }
        }
    }

    fn post_update_state(&mut self, source_event_id: &mut EventId, cx: &mut nexosim::model::Context<Self>) -> impl Future<Output = ()> + Send {
        async move {
            match self.time_to_next_event {
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

    fn log(&mut self, now: MonotonicTime, source_event_id: EventId, details: Self::LogDetailsType) -> impl Future<Output = EventId> {
        async move {
            let new_event_id = EventId(format!("{}_{:06}", self.element_code, self.next_event_index));
            let log = VectorProcessLog {
                time: now.to_chrono_date_time(0).unwrap().to_string(),
                event_id: new_event_id.clone(),
                source_event_id,
                element_name: self.element_name.clone(),
                element_type: self.element_type.clone(),
                event: details,
            };
            self.log_emitter.send(log.clone()).await;
            self.next_event_index += 1;

            new_event_id
        }
    }
}


/**
 * Source
 */
pub struct VectorSource<
    InternalResourceType: Clone + Send + 'static,
    SendType: Clone + Send + 'static,
> {
    pub element_name: String,
    pub element_code: String,
    pub element_type: String,
    pub req_downstream: Requestor<(), VectorStockState>,
    pub push_downstream: Output<(SendType, EventId)>,
    pub process_state: Option<(Duration, InternalResourceType)>,
    pub process_quantity_distr: Distribution,
    pub process_time_distr: Distribution,
    pub source_vector: InternalResourceType,
    time_to_next_event: Option<Duration>,
    scheduled_event: Option<(MonotonicTime, ActionKey)>,
    next_event_index: u64,
    pub log_emitter: Output<VectorProcessLog<InternalResourceType>>,
    pub previous_check_time: MonotonicTime,
}

impl<InternalResourceType: Clone + Default + Send, SendType: Clone + Send> Default for VectorSource<InternalResourceType, SendType> {
    fn default() -> Self {
        VectorSource {
            element_name: String::new(),
            element_code: String::new(),
            element_type: String::new(),
            req_downstream: Requestor::default(),
            push_downstream: Output::default(),
            process_state: None,
            process_quantity_distr: Distribution::default(),
            process_time_distr: Distribution::default(),
            source_vector: InternalResourceType::default(),
            time_to_next_event: None,
            scheduled_event: None,
            next_event_index: 0,
            log_emitter: Output::default(),
            previous_check_time: MonotonicTime::EPOCH,
        }
    }
}

impl<
    InternalResourceType: Clone + Send + 'static,
    SendType: Clone + Send + 'static
> Model for VectorSource<InternalResourceType, SendType> where Self: Process {
    fn init(mut self, ctx: &mut Context<Self>) -> impl Future<Output = InitializedModel<Self>> + Send {
        async move {
            let source_event_id = EventId(format!("{}_{:06}", self.element_code, self.next_event_index));
            self.update_state(source_event_id, ctx).await;
            self.into()
        }
    }
}

impl<InternalResourceType: Send + 'static + Clone + Default, SendType: Send + 'static + Clone + Default> VectorSource<InternalResourceType, SendType> {
    pub fn new() -> Self {
        Default::default()
    }
    
    pub fn with_name(self, name: String) -> Self {
        Self {
            element_name: name,
            ..self
        }
    }

    pub fn with_code(self, code: String) -> Self {
        Self {
            element_code: code,
            ..self
        }
    }

    pub fn with_process_quantity_distr(self, process_quantity_distr: Distribution) -> Self {
        Self {
            process_quantity_distr,
            ..self
        }
    }

    pub fn with_process_time_distr(self, process_time_distr: Distribution) -> Self {
        Self {
            process_time_distr,
            ..self
        }
    }

    pub fn with_source_vector(self, source_vector: InternalResourceType) -> Self {
        Self {
            source_vector,
            ..self
        }
    }
}

impl<T: Clone + Send + 'static> Process for VectorSource<T, T>
where
    T: ResourceTotal<f64> + ResourceMultiply<f64>
{
    type LogDetailsType = VectorProcessLogType<T>;

    fn pre_update_state(&mut self, source_event_id: &mut EventId, cx: &mut Context<Self>) -> impl Future<Output = ()> {
        async move {
            if let Some((scheduled_time, _)) = self.scheduled_event.as_ref() {
                if *scheduled_time <= cx.time() {
                    self.scheduled_event = None;
                }
            }
        }
    }

    fn update_state_impl(&mut self, source_event_id: &mut EventId, cx: &mut nexosim::model::Context<Self>) -> impl Future<Output = ()>{
        async move {
            let time = cx.time();

            if let Some((mut process_time_left, resource)) = self.process_state.take() {
                let duration_since_prev_check = cx.time().duration_since(self.previous_check_time);
                process_time_left = process_time_left.saturating_sub(duration_since_prev_check);
                if process_time_left.is_zero() {
                    *source_event_id = self.log(time, source_event_id.clone(), VectorProcessLogType::ProcessSuccess { quantity: resource.total(), vector: resource.clone() }).await;
                    self.push_downstream.send((resource.clone(), source_event_id.clone())).await;
                } else {
                    self.process_state = Some((process_time_left, resource));
                }
            }
            match self.process_state {
                None => {
                    let ds_state = self.req_downstream.send(()).await.next();
                    match ds_state {
                        Some(VectorStockState::Full {..}) => {
                            *source_event_id = self.log(time, source_event_id.clone(), VectorProcessLogType::ProcessFailure { reason: "Downstream is full" }).await;
                            self.time_to_next_event = None;
                        },
                        Some(VectorStockState::Normal {..}) | Some(VectorStockState::Empty {..}) => {
                            let process_quantity = self.process_quantity_distr.sample();
                            if self.source_vector.total() <= 0. {
                                panic!("Source vector has total 0 or negative ({}), cannot process!", self.source_vector.total());
                            }
                            let mut created = self.source_vector.clone();
                            
                            created.multiply(process_quantity / created.total());
                            let process_duration_secs = self.process_time_distr.sample();
                            self.process_state = Some((Duration::from_secs_f64(process_duration_secs), created.clone()));
                            *source_event_id = self.log(time, source_event_id.clone(), VectorProcessLogType::ProcessStart { quantity: process_quantity, vector: created }).await;
                            self.time_to_next_event = Some(Duration::from_secs_f64(process_duration_secs));
                        },
                        None => {
                            *source_event_id = self.log(time, source_event_id.clone(), VectorProcessLogType::ProcessFailure { reason: "Downstream is not connected" }).await;
                            self.time_to_next_event = None;
                        },
                    }
                },
                Some((time, _)) => {
                    self.time_to_next_event = Some(time);
                }
            }
        }
    }

    fn post_update_state(&mut self, source_event_id: &mut EventId, cx: &mut nexosim::model::Context<Self>) -> impl Future<Output = ()> {
        async move {
            match self.time_to_next_event {
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

    fn log(&mut self, now: MonotonicTime, source_event_id: EventId, details: Self::LogDetailsType) -> impl Future<Output = EventId> {
        async move {
            let new_event_id = EventId(format!("{}_{:06}", self.element_code, self.next_event_index));
            let log = VectorProcessLog {
                time: now.to_chrono_date_time(0).unwrap().to_string(),
                event_id: new_event_id.clone(),
                source_event_id,
                element_name: self.element_name.clone(),
                element_type: self.element_type.clone(),
                event: details,
            };
            self.log_emitter.send(log.clone()).await;
            self.next_event_index += 1;

            new_event_id
        }
    }
}

/**
 * Sink
 */

pub struct VectorSink<
    ReceiveParameterType: Clone + Send + 'static,
    ReceiveType: Clone + Send + 'static,
    InternalResourceType: Clone + Send + 'static,
> {
    pub element_name: String,
    pub element_code: String,
    pub element_type: String,
    pub req_upstream: Requestor<(), VectorStockState>,
    pub withdraw_upstream: Requestor<(ReceiveParameterType, EventId), ReceiveType>,
    pub process_state: Option<(Duration, InternalResourceType)>,
    pub process_quantity_distr: Distribution,
    pub process_time_distr: Distribution,
    time_to_next_event: Option<Duration>,
    scheduled_event: Option<(MonotonicTime, ActionKey)>,
    next_event_index: u64,
    pub log_emitter: Output<VectorProcessLog<InternalResourceType>>,
    pub previous_check_time: MonotonicTime,
}

impl<
    ReceiveParameterType: Clone + Send + 'static,
    ReceiveType: Clone + Send + 'static,
    InternalResourceType: Clone + Send + 'static,
> Model for VectorSink<ReceiveParameterType, ReceiveType, InternalResourceType> where Self: Process {
    fn init(mut self, ctx: &mut Context<Self>) -> impl Future<Output = InitializedModel<Self>> + Send {
        async move {
            let source_event_id = EventId(format!("{}_{:06}", self.element_code, self.next_event_index));
            self.update_state(source_event_id, ctx).await;
            self.into()
        }
    }
}

impl<
    ReceiveParameterType: Clone + Send + Default + 'static,
    ReceiveType: Clone + Send + 'static,
    InternalResourceType: Clone + Send + 'static,
> Default for VectorSink<ReceiveParameterType, ReceiveType, InternalResourceType> {
    fn default() -> Self {
        VectorSink {
            element_name: String::new(),
            element_code: String::new(),
            element_type: String::new(),
            req_upstream: Requestor::default(),
            withdraw_upstream: Requestor::default(),
            process_state: None,
            process_quantity_distr: Distribution::default(),
            process_time_distr: Distribution::default(),
            time_to_next_event: None,
            scheduled_event: None,
            next_event_index: 0,
            log_emitter: Output::default(),
            previous_check_time: MonotonicTime::EPOCH,
        }
    }
}

impl<
    ReceiveParameterType: Clone + Send + 'static,
    ReceiveType: Clone + Send + 'static,
    InternalResourceType: Clone + Send + 'static,
> VectorSink<ReceiveParameterType, ReceiveType, InternalResourceType> where Self: Default {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn with_name(self, name: String) -> Self {
        Self {
            element_name: name,
            ..self
        }
    }

    pub fn with_code(self, code: String) -> Self {
        Self {
            element_code: code,
            ..self
        }
    }

    pub fn with_process_quantity_distr(self, process_quantity_distr: Distribution) -> Self {
        Self {
            process_quantity_distr,
            ..self
        }
    }

    pub fn with_process_time_distr(self, process_time_distr: Distribution) -> Self {
        Self {
            process_time_distr,
            ..self
        }
    }
}

impl<T: Send + 'static + Clone + Default> Process for VectorSink<f64, T, T>
where
    T: ResourceTotal<f64>,
{
    type LogDetailsType = VectorProcessLogType<T>;

    fn pre_update_state(&mut self, source_event_id: &mut EventId, cx: &mut Context<Self>) -> impl Future<Output = ()> {
        async move {
            if let Some((scheduled_time, _)) = self.scheduled_event.as_ref() {
                if *scheduled_time <= cx.time() {
                    self.scheduled_event = None;
                }
            }
        }
    }

    fn update_state_impl(&mut self, source_event_id: &mut EventId, cx: &mut nexosim::model::Context<Self>) -> impl Future<Output = ()> {
        async move {
            let time = cx.time();

            if let Some((mut process_time_left, resource)) = self.process_state.take() {
                let duration_since_prev_check = cx.time().duration_since(self.previous_check_time);
                process_time_left = process_time_left.saturating_sub(duration_since_prev_check);
                if process_time_left.is_zero() {
                    *source_event_id = self.log(time, source_event_id.clone(), VectorProcessLogType::ProcessSuccess { quantity: resource.total(), vector: resource.clone() }).await;
                } else {
                    self.process_state = Some((process_time_left, resource));
                }
            }
            match self.process_state {
                None => {
                    let us_state = self.req_upstream.send(()).await.next();
                    match us_state {
                        Some(VectorStockState::Normal {..}) | Some(VectorStockState::Full {..}) => {
                            let process_quantity = self.process_quantity_distr.sample();
                            *source_event_id = self.log(time, source_event_id.clone(), VectorProcessLogType::WithdrawRequest).await;
                            let withdrawn = self.withdraw_upstream.send((process_quantity, source_event_id.clone())).await.next().unwrap();
                            let process_duration_secs = self.process_time_distr.sample();
                            self.process_state = Some((Duration::from_secs_f64(process_duration_secs), withdrawn.clone()));
                            self.log(time, source_event_id.clone(), VectorProcessLogType::ProcessStart { quantity: process_quantity, vector: withdrawn }).await;
                            self.time_to_next_event = Some(Duration::from_secs_f64(process_duration_secs));
                        },
                        Some(VectorStockState::Empty {..}) => {
                        }
                        None => {
                            self.log(time, source_event_id.clone(), VectorProcessLogType::ProcessFailure { reason: "Upstream is not connected" }).await;
                            self.time_to_next_event = None;
                        },
                        _ => {
                            self.log(time, source_event_id.clone(), VectorProcessLogType::ProcessFailure { reason: "Upstream is empty" }).await;
                            self.time_to_next_event = None;
                        }
                    }
                },
                Some((time, _)) => {
                    self.time_to_next_event = Some(time);
                }
            }
        }
    }

    fn post_update_state(&mut self, source_event_id: &mut EventId, cx: &mut nexosim::model::Context<Self>) -> impl Future<Output = ()> + Send {
        async move {
            match self.time_to_next_event {
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

    fn log(&mut self, now: MonotonicTime, source_event_id: EventId, details: Self::LogDetailsType) -> impl Future<Output = EventId> {
        async move {
            let new_event_id = EventId(format!("{}_{:06}", self.element_code, self.next_event_index));
            let log = VectorProcessLog {
                time: now.to_chrono_date_time(0).unwrap().to_string(),
                event_id: new_event_id.clone(),
                source_event_id,
                element_name: self.element_name.clone(),
                element_type: self.element_type.clone(),
                event: details,
            };
            self.log_emitter.send(log.clone()).await;
            self.next_event_index += 1;

            new_event_id
        }
    }
}
