use std::{fmt::Debug, time::Duration};
use serde::Serialize;

use crate::{common::Distribution, delays::DelayModes, nexosim::{Output, Requestor, ActionKey, MonotonicTime, Context, Model}};

#[derive(Debug, Clone)]
pub struct EventId {}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum BasicEnvironmentState {
    Normal,
    Stopped,
}

pub enum VectorStockState {
    Normal { occupied: f64, empty: f64 },
    Full { occupied: f64, empty: f64 },
    Empty { occupied: f64, empty: f64 },
}

pub enum VectorProcessLogType<T: ContinuousResource> {
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

#[derive(Clone)]
pub struct VectorProcessLog {}

pub struct DefaultProcess<
    // ReceiveParameterType: Clone + Send + Debug + 'static,
    // ReceiveType: Clone + Send + Debug + 'static,
    ResourceType: Clone + Send + Debug + 'static,
    // SendType: Clone + Send + Debug + 'static,
    // VectorStockState: Clone + Send + Debug + 'static,
    // VectorProcessLog: Clone + Send + Debug + 'static,
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
    pub log_emitter: Output<VectorProcessLog>,

    // Configuration
    pub process_quantity_distr: Distribution,
    pub process_time_distr: Distribution,
    pub delay_modes: DelayModes,

    // Runtime State
    pub process_state: Option<(Duration, ResourceType)>,
    pub env_state: BasicEnvironmentState,
    
    // Internals
    time_to_next_process_event: Option<Duration>,
    time_to_next_delay_event: Option<Duration>,
    scheduled_event: Option<(MonotonicTime, ActionKey)>,
    next_event_index: u64,
    previous_check_time: MonotonicTime,
}

impl<
    // ReceiveParameterType: Clone + Send + Debug,
    // ReceiveType: Clone + Send + Debug,
    ResourceType: Clone + Send + Debug,
    // SendType: Clone + Send + Debug,
    // VectorStockState: Clone + Send + Debug,
    // VectorProcessLog: Clone + Send + Debug,
> Model for DefaultProcess<
    // ReceiveParameterType,
    // ReceiveType,
    ResourceType,
    // SendType,
    // VectorStockState,
    // VectorProcessLog,
> {}

impl<
    // ReceiveParameterType: Clone + Send + Debug,
    // ReceiveType: Clone + Send + Debug,
    ResourceType: Clone + Send + Debug,
    // SendType: Clone + Send + Debug,
    // VectorStockState: Clone + Send + Debug,
    // VectorProcessLog: Clone + Send + Debug,
> Default for DefaultProcess<
    // ReceiveParameterType,
    // ReceiveType,
    ResourceType,
    // SendType,
    // VectorStockState,
    // VectorProcessLog,
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
    // ReceiveParameterType: Clone + Send + Debug, == f64
    // ReceiveType: Clone + Send + Debug, == ResourceType
    ResourceType: Clone + Send + Debug + ContinuousResource,
    // SendType: Clone + Send + Debug, // == ResourceType
    // VectorStockState: Clone + Send + Debug,
    // VectorProcessLog: Clone + Send + Debug,
> DefaultProcess<
    // ReceiveParameterType,
    // ReceiveType,
    ResourceType,
    // SendType,
    // VectorStockState,
    // VectorProcessLog,
> {
    fn update_state(
        &mut self, mut source_event_id: EventId, cx: &mut Context<Self>
    ) -> impl Future<Output = ()> + Send where Self: Model {
        async move {
            // Update variables from elapsed time
            if let Some((scheduled_time, _)) = self.scheduled_event.as_ref() {
                if *scheduled_time <= cx.time() {
                    self.scheduled_event = None;
                }
            }

            let time = cx.time();
            let duration_since_prev_check = cx.time().duration_since(self.previous_check_time);
            {
                let is_in_delay = self.delay_modes.active_delay().is_some();
                let is_in_process = self.process_state.is_some() && !is_in_delay;
                let is_env_blocked = matches!(self.env_state, BasicEnvironmentState::Stopped);

                // Decrement process time counter (if not delayed or env blocked)
                if !(is_in_delay || is_env_blocked) {
                    if let Some((mut process_time_left, resource)) = self.process_state.take() {
                        process_time_left = process_time_left.saturating_sub(duration_since_prev_check);
                        if process_time_left.is_zero() {
                            source_event_id = self.log(time, source_event_id.clone(), VectorProcessLogType::ProcessSuccess { quantity: resource.total(), vector: resource.clone() }).await;
                            self.push_downstream.send((resource.clone(), source_event_id.clone())).await;
                        } else {
                            self.process_state = Some((process_time_left, resource));
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
                            source_event_id = self.log(time, source_event_id.clone(), VectorProcessLogType::DelayEnd { delay_name: delay_name.clone() }).await;
                        }
                        if let Some(delay_name) = &delay_transition.to {
                            source_event_id = self.log(time, source_event_id.clone(), VectorProcessLogType::DelayStart { delay_name: delay_name.clone() }).await;
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
                        source_event_id = self.log(time, source_event_id.clone(), VectorProcessLogType::ProcessStopped { reason: "Stopped by environment" }).await;
                        self.env_state = BasicEnvironmentState::Stopped;
                    },
                    (BasicEnvironmentState::Stopped, BasicEnvironmentState::Normal) => {
                        source_event_id = self.log(time, source_event_id.clone(), VectorProcessLogType::ProcessContinue { reason: "Resumed by environment" }).await;
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
                    let us_state = self.req_upstream.send(()).await.next();
                    let ds_state = self.req_downstream.send(()).await.next();
                    match (&us_state, &ds_state) {
                        (
                            Some(VectorStockState::Normal {..}) | Some(VectorStockState::Full {..}),
                            Some(VectorStockState::Empty {..}) | Some(VectorStockState::Normal {..}),
                        ) => {
                            let process_quantity = self.process_quantity_distr.sample();
                            source_event_id = self.log(time, source_event_id.clone(), VectorProcessLogType::WithdrawRequest).await;
                            let moved = self.withdraw_upstream.send((process_quantity, source_event_id.clone())).await.next().unwrap();
                            let process_duration_secs = self.process_time_distr.sample();
                            self.process_state = Some((Duration::from_secs_f64(process_duration_secs), moved.clone()));
                            source_event_id = self.log(time, source_event_id.clone(), VectorProcessLogType::ProcessStart { quantity: process_quantity, vector: moved }).await;
                            self.time_to_next_process_event = Some(Duration::from_secs_f64(process_duration_secs));
                        },
                        (Some(VectorStockState::Empty {..} ), _) => {
                            source_event_id = self.log(time, source_event_id.clone(), VectorProcessLogType::ProcessFailure { reason: "Upstream is empty" }).await;
                            self.time_to_next_process_event = None;
                        },
                        (None, _) => {
                            source_event_id = self.log(time, source_event_id.clone(), VectorProcessLogType::ProcessFailure { reason: "Upstream is not connected" }).await;
                            self.time_to_next_process_event = None;
                        },
                        (_, None) => {
                            source_event_id = self.log(time, source_event_id.clone(), VectorProcessLogType::ProcessFailure { reason: "Downstream is not connected" }).await;
                            self.time_to_next_process_event = None;
                        },
                        (_, Some(VectorStockState::Full {..} )) => {
                            source_event_id = self.log(time, source_event_id.clone(), VectorProcessLogType::ProcessFailure { reason: "Downstream is full" }).await;
                            self.time_to_next_process_event = None;
                        },
                    }
                },
                (Some((time, _)), false) => {
                    self.time_to_next_process_event = Some(*time);
                },
                (_, true) => {
                    self.time_to_next_process_event = self.delay_modes.active_delay().map(|(_, delay_state)| *delay_state);
                }
            }

            // Schedule next event
            if self.process_state.is_some() || has_active_delay || !is_env_stopped {
                self.time_to_next_delay_event = self.delay_modes.get_next_event().map(|(_, delay_state)| delay_state.as_duration());
            } else {
                self.time_to_next_delay_event = None;
            }
            let time_to_next_event = [self.time_to_next_delay_event, self.time_to_next_process_event].into_iter().flatten().min();
            match time_to_next_event {
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
                                let new_event_key =  cx.schedule_keyed_event(next_time, Self::update_state, source_event_id.clone()).unwrap();
                                self.scheduled_event = Some((next_time, new_event_key));
                            } else {
                                // Put the event back
                                self.scheduled_event = Some((scheduled_time, action_key));
                            }
                        } else {
                            let new_event_key =  cx.schedule_keyed_event(next_time, Self::update_state, source_event_id.clone()).unwrap();
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
            let new_event_id: EventId = EventId(format!("{}_{:06}", self.element_code, self.next_event_index));
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

pub trait Projectable<T: ContinuousResource> {
    fn project(self) -> T;
}

pub trait StockState {}
pub struct ExampleStockState {}
impl StockState for ExampleStockState {}

pub struct DefaultStock<T, S> where T: ContinuousResource, S: StockState {
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
    prev_state: Option<VectorStockState>,
    next_event_id: u64,
}

trait WithStockState<S> {
    fn get_state(&mut self) -> S;
}

impl DefaultStock<T, S> where T: ContinuousResource, S: StockState {
    // type StockState = S;
    // type LogDetailsType = VectorStockLogType<T>;

    fn get_state(&mut self) -> S {
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

    fn get_previous_state(&mut self) -> &Option<S> {
        &self.prev_state
    }
    fn set_previous_state(&mut self) {
        self.prev_state = Some(self.get_state());
    }
    fn get_resource(&self) -> &T {
        &self.resource
    }

    fn add_impl(
        &mut self,
        payload: &mut (T, EventId),
        cx: &mut ::nexosim::model::Context<Self>
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

    fn remove_impl(
        &mut self,
        payload: &mut (f64, EventId),
        cx: &mut ::nexosim::model::Context<Self>
    ) -> impl Future<Output=T> {
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

    fn emit_change(&mut self, payload: (Self::StockState, EventId), cx: &mut nexosim::model::Context<Self>) -> impl Future<Output=()> {
        async move {
            let nm = self.log(cx.time(), payload.1, VectorStockLogType::StateChange { new_state: payload.0 }).await;
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

pub trait ContinuousResource {
    fn add(&mut self, arg: Self);
    fn remove(&mut self, arg: Self) -> Self;
    fn multiply(&mut self, arg: f64);
    fn total(&self) -> f64;
    fn remove_all(&mut self) -> Self;
}

impl ContinuousResource for f64 {
    fn add(&mut self, arg: Self) {
        *self += arg;
    }

    fn remove(&mut self, arg: Self) -> Self {
        let removed = *self;
        *self -= arg;
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

impl ContinuousResource for [f64; N] {
    fn add(&mut self, arg: Self) {
        for (a, b) in self.iter_mut().zip(arg.iter()) {
            *a += *b;
        }
    }

    fn remove(&mut self, arg: Self) -> Self {
        let removed = *self;
        for (a, b) in self.iter_mut().zip(arg.iter()) {
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

pub trait Connect<A, B> {
    fn connect(&mut self, a: &mut A, b: &mut B) -> Result<(), String>;
}

pub struct Connection;

impl<T: ContinuousResource, U: StockState> Connect<DefaultProcess<T>, DefaultStock<T, U>> for Connection {
    fn connect(&mut self, a: &mut DefaultProcess<T>, b: &mut DefaultStock<T, U>) -> Result<(), String> {
        // Example connection logic
        Ok(())
    }
}