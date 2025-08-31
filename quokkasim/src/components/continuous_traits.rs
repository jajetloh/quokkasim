use serde::Serialize;
use std::{fmt::Debug, time::Duration};

use crate::{
    common::{EventId, StockState, ToLogRecord},
    components::environment::BasicEnvironmentState,
    delays::DelayModes,
    distributions::Distribution,
    nexosim::{ActionKey, Context, Model, MonotonicTime, Output, Requestor},
    prelude::ContinuousStockState,
};

pub trait ContinuousArithmetic {
    fn add(&mut self, arg: Self);
    fn remove<T>(&mut self, arg: T) -> Self
    where
        Self: Projectable<T>;
    fn multiply(&mut self, arg: f64);
    fn total(&self) -> f64;
    fn remove_all(&mut self) -> Self;
}

pub trait Projectable<T> {
    fn project(self, arg: T) -> Self;
}

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

    fn remove<T>(&mut self, arg: T) -> Self
    where
        Self: Projectable<T>,
    {
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

    fn remove<T>(&mut self, arg: T) -> Self
    where
        Self: Projectable<T>,
    {
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

pub trait ContinuousResource: ContinuousArithmetic + Clone + Send + Debug + Serialize {}
impl<T> ContinuousResource for T where T: ContinuousArithmetic + Clone + Send + Debug + Serialize {}

pub trait Process<
    ResourceType: ContinuousResource + 'static,
    LogRecordType: Clone + Send + 'static,
    LogDetailsType: Clone + Send + 'static,
> where
    Self: Model,
    Self: ToLogRecord<LogDetailsType, LogRecordType>,
{
    fn update_state(
        &mut self,
        mut source_event_id: EventId,
        mut cx: &mut Context<Self>,
    ) -> impl Future<Output = ()> + Send {
        async move {
            self.update_state_since_last_update(&mut source_event_id, &mut cx)
                .await;
            self.update_state_decision_logic(&mut source_event_id, &mut cx)
                .await;
            self.update_state_next_event(&mut source_event_id, &mut cx)
                .await;
        }
    }

    fn update_state_since_last_update(
        &mut self,
        source_event_id: &mut EventId,
        cx: &mut Context<Self>,
    ) -> impl Future<Output = ()> + Send {
        async move {
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
                        process_time_left =
                            process_time_left.saturating_sub(duration_since_prev_check);
                        if process_time_left.is_zero() {
                            *source_event_id = self
                                .log(
                                    time,
                                    source_event_id.clone(),
                                    self.log_type_process_success(
                                        resource.total(),
                                        resource.clone(),
                                    ),
                                )
                                .await;
                            self.push_downstream()
                                .send((resource.clone(), source_event_id.clone()))
                                .await;
                        } else {
                            *self.process_state() = Some((process_time_left, resource));
                        }
                    }
                }

                // Only case we don't update state here is if no delay is if we don't want the delay counters to decrement,
                // which is only the case if we're not processing and not in a delay - i.e. time-until-delay counters only decrement
                // when a process is active
                if !is_env_blocked && (is_in_delay || is_in_process) {
                    let delay_transition =
                        self.delay_modes().update_state(duration_since_prev_check);
                    if delay_transition.has_changed() {
                        if let Some(delay_name) = &delay_transition.from {
                            *source_event_id = self
                                .log(
                                    time,
                                    source_event_id.clone(),
                                    self.log_type_delay_end(delay_name.clone()),
                                )
                                .await;
                        }
                        if let Some(delay_name) = &delay_transition.to {
                            *source_event_id = self
                                .log(
                                    time,
                                    source_event_id.clone(),
                                    self.log_type_delay_start(delay_name.clone()),
                                )
                                .await;
                        }
                    }
                }
            }
        }
    }

    fn update_state_decision_logic(
        &mut self,
        source_event_id: &mut EventId,
        cx: &mut Context<Self>,
    ) -> impl Future<Output = ()> + Send {
        async move {
            let time = cx.time();
            {
                let new_env_state = match self.req_environment().send(()).await.next() {
                    Some(x) => x,
                    None => BasicEnvironmentState::Normal, // Assume always normal operation if no environment state connected
                };
                match (&self.env_state(), &new_env_state) {
                    (BasicEnvironmentState::Normal, BasicEnvironmentState::Stopped) => {
                        *source_event_id = self
                            .log(
                                time,
                                source_event_id.clone(),
                                self.log_type_process_stopped("Stopped by environment"),
                            )
                            .await;
                        *self.env_state() = BasicEnvironmentState::Stopped;
                    }
                    (BasicEnvironmentState::Stopped, BasicEnvironmentState::Normal) => {
                        *source_event_id = self
                            .log(
                                time,
                                source_event_id.clone(),
                                self.log_type_process_continue("Resumed by environment"),
                            )
                            .await;
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

                    match (&us_state, &ds_state) {
                        (
                            Some(ContinuousStockState::Normal { .. })
                            | Some(ContinuousStockState::Full { .. }),
                            Some(ContinuousStockState::Empty { .. })
                            | Some(ContinuousStockState::Normal { .. }),
                        ) => {
                            let process_quantity = self.process_quantity_distr().sample();
                            *source_event_id = self
                                .log(
                                    time,
                                    source_event_id.clone(),
                                    self.log_type_withdraw_request(),
                                )
                                .await;
                            let moved = self
                                .withdraw_upstream()
                                .send((process_quantity, source_event_id.clone()))
                                .await
                                .next()
                                .unwrap();
                            let process_duration_secs = self.process_time_distr().sample();
                            *self.process_state() = Some((
                                Duration::from_secs_f64(process_duration_secs),
                                moved.clone(),
                            ));
                            *source_event_id = self
                                .log(
                                    time,
                                    source_event_id.clone(),
                                    self.log_type_process_start(process_quantity, moved),
                                )
                                .await;
                            *self.time_to_next_process_event() =
                                Some(Duration::from_secs_f64(process_duration_secs));
                        }
                        (Some(ContinuousStockState::Empty { .. }), _) => {
                            *source_event_id = self
                                .log(
                                    time,
                                    source_event_id.clone(),
                                    self.log_type_process_failure("Upstream is empty"),
                                )
                                .await;
                            *self.time_to_next_process_event() = None;
                        }
                        (None, _) => {
                            *source_event_id = self
                                .log(
                                    time,
                                    source_event_id.clone(),
                                    self.log_type_process_failure("Upstream is not connected"),
                                )
                                .await;
                            *self.time_to_next_process_event() = None;
                        }
                        (_, None) => {
                            *source_event_id = self
                                .log(
                                    time,
                                    source_event_id.clone(),
                                    self.log_type_process_failure("Downstream is not connected"),
                                )
                                .await;
                            *self.time_to_next_process_event() = None;
                        }
                        (_, Some(ContinuousStockState::Full { .. })) => {
                            *source_event_id = self
                                .log(
                                    time,
                                    source_event_id.clone(),
                                    self.log_type_process_failure("Downstream is full"),
                                )
                                .await;
                            *self.time_to_next_process_event() = None;
                        }
                    }
                }
                (Some((time, _)), false) => {
                    *self.time_to_next_process_event() = Some(*time);
                }
                (_, true) => {
                    *self.time_to_next_process_event() = self
                        .delay_modes()
                        .active_delay()
                        .map(|(_, delay_state)| *delay_state);
                }
            }
        }
    }

    fn update_state_next_event(
        &mut self,
        source_event_id: &mut EventId,
        cx: &mut Context<Self>,
    ) -> impl Future<Output = ()> + Send {
        async move {
            let is_env_stopped = matches!(self.env_state(), BasicEnvironmentState::Stopped);
            let has_active_delay = self.delay_modes().active_delay().is_some() || is_env_stopped;

            if self.process_state().is_some() || has_active_delay || !is_env_stopped {
                *self.time_to_next_delay_event() = self
                    .delay_modes()
                    .get_next_event()
                    .map(|(_, delay_state)| delay_state.as_duration());
            } else {
                *self.time_to_next_delay_event() = None;
            }
            let time_to_next_event = [
                self.time_to_next_delay_event().clone(),
                self.time_to_next_process_event().clone(),
            ]
            .into_iter()
            .flatten()
            .min();
            match time_to_next_event {
                None => {}
                Some(time_until_next) => {
                    if time_until_next.is_zero() {
                        panic!("Time until next event is zero!");
                    } else {
                        let next_time = cx.time() + time_until_next;

                        // Schedule event if sooner. If so, cancel previous event.
                        if let Some((scheduled_time, action_key)) = self.scheduled_event().take() {
                            if next_time < scheduled_time {
                                action_key.cancel();
                                let new_event_key = cx
                                    .schedule_keyed_event(
                                        next_time,
                                        Self::update_state,
                                        source_event_id.clone(),
                                    )
                                    .unwrap();
                                *self.scheduled_event() = Some((next_time, new_event_key));
                            } else {
                                // Put the event back
                                *self.scheduled_event() = Some((scheduled_time, action_key));
                            }
                        } else {
                            let new_event_key = cx
                                .schedule_keyed_event(
                                    next_time,
                                    Self::update_state,
                                    source_event_id.clone(),
                                )
                                .unwrap();
                            *self.scheduled_event() = Some((next_time, new_event_key));
                        }
                    };
                }
            };
            *self.previous_check_time() = cx.time();
        }
    }

    fn log<D: Into<LogDetailsType> + Send>(
        &mut self,
        now: MonotonicTime,
        source_event_id: EventId,
        details: D,
    ) -> impl Future<Output = EventId> + Send {
        async move {
            let event_id = self.get_next_event_id();
            let log = self.to_record(now, source_event_id.clone(), event_id.clone(), details.into());
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
    fn req_upstream(&mut self) -> &mut Requestor<(), ContinuousStockState>;
    fn withdraw_upstream(&mut self) -> &mut Requestor<(f64, EventId), ResourceType>;
    fn req_downstream(&mut self) -> &mut Requestor<(), ContinuousStockState>;
    fn push_downstream(&mut self) -> &mut Output<(ResourceType, EventId)>;
    fn process_quantity_distr(&mut self) -> &mut Distribution;
    fn process_time_distr(&mut self) -> &mut Distribution;
    fn time_to_next_process_event(&mut self) -> &mut Option<Duration>;
    fn time_to_next_delay_event(&mut self) -> &mut Option<Duration>;

    fn log_type_withdraw_request(&self) -> LogDetailsType;
    fn log_type_process_start(&self, quantity: f64, resource: ResourceType) -> LogDetailsType;
    fn log_type_process_success(&self, quantity: f64, resource: ResourceType) -> LogDetailsType;
    fn log_type_process_failure(&self, reason: &'static str) -> LogDetailsType;
    fn log_type_process_stopped(&self, reason: &'static str) -> LogDetailsType;
    fn log_type_process_continue(&self, reason: &'static str) -> LogDetailsType;
    fn log_type_delay_start(&self, delay_name: String) -> LogDetailsType;
    fn log_type_delay_end(&self, delay_name: String) -> LogDetailsType;
}

pub trait Stock<
    ResourceType: ContinuousResource + 'static,
    StateType: StockState + Clone + Send + 'static,
    LogRecordType: Clone + Send + 'static,
    LogDetailsType: Clone + Send + 'static,
> where
    Self: Model,
    Self: ToLogRecord<LogDetailsType, LogRecordType>,
{
    fn get_state(&mut self) -> StateType;

    fn get_state_async(&mut self, _: (), _: &mut Context<Self>) -> impl Future<Output = StateType> {
        async move {
            self.get_state()
        }
    }

    fn log_emitter(&mut self) -> &mut Output<LogRecordType>;

    fn get_next_event_id(&mut self) -> EventId;

    fn previous_state(&mut self) -> &mut Option<StateType>;
    fn resource(&mut self) -> &mut ResourceType;
    fn state_emitter(&mut self) -> &mut Output<EventId>;

    fn add(&mut self, payload: (ResourceType, EventId), cx: &mut Context<Self>) -> impl Future<Output = ()> + Send {
        async move {
            *self.previous_state() = Some(self.get_state().clone());
            self.resource().add(payload.0.clone());
            let total = self.resource().total();
            let event_id = self
                .log(
                    cx.time(),
                    payload.1.clone(),
                    self.log_type_add(total, payload.0.clone())
                )
                .await;

            let previous_state = self.previous_state().clone();
            let current_state = self.get_state().clone();
            if previous_state.is_none() || !previous_state.as_ref().unwrap().is_same_state(&current_state)
            {
                // Send 1ns in future to avoid infinite loops with processes
                let next_time = cx.time() + Duration::from_nanos(1);
                cx.schedule_event(next_time, Self::emit_change, (current_state.clone(), event_id)).unwrap();
            }
            *self.previous_state() = Some(current_state);
        }
    }

    fn remove<T: Send>(&mut self, payload: (T, EventId), cx: &mut Context<Self>) -> impl Future<Output = ResourceType> + Send where ResourceType: Projectable<T> {
        async move {
            *self.previous_state() = Some(self.get_state().clone());
            let removed = self.resource().remove(payload.0);
            let total = self.resource().total();
            let event_id = self.log(
                    cx.time(),
                    payload.1.clone(),
                    self.log_type_remove(total, removed.clone())
                )
                .await;

            let previous_state = self.previous_state().clone();
            let current_state = self.get_state().clone();
            if previous_state.is_none() || !previous_state.as_ref().unwrap().is_same_state(&current_state)
            {
                // Send 1ns in future to avoid infinite loops with processes
                let next_time = cx.time() + Duration::from_nanos(1);
                cx.schedule_event(next_time, Self::emit_change, (current_state.clone(), event_id)).unwrap();
            }
            *self.previous_state() = Some(current_state);
            removed
        }
    }

    fn remove_void<T: Send>(&mut self, payload: (T, EventId), cx: &mut Context<Self>) -> impl Future<Output = ()> + Send where ResourceType: Projectable<T> {
        async move {
            self.remove(payload, cx).await;
        }
    }

    fn emit_change(&mut self, payload: (StateType, EventId), cx: &mut Context<Self>) -> impl Future<Output = ()> + Send {
        async move {
            let nm = self
                .log(
                    cx.time(),
                    payload.1,
                    self.log_type_state_change(payload.0.clone()),
                )
                .await;
            self.state_emitter().send(nm).await;
        }
    }

    fn log<D: Into<LogDetailsType> + Send>(
        &mut self,
        now: MonotonicTime,
        source_event_id: EventId,
        details: D,
    ) -> impl Future<Output = EventId> + Send {
        async move {
            let event_id = self.get_next_event_id();
            let log = self.to_record(now, source_event_id.clone(), event_id.clone(), details.into());
            self.log_emitter().send(log.clone()).await;
            event_id
        }
    }

    fn log_type_add(&self, balance: f64, resource: ResourceType) -> LogDetailsType;
    fn log_type_remove(&self, balance: f64, resource: ResourceType) -> LogDetailsType;
    fn log_type_state_change(&self, new_state: StateType) -> LogDetailsType;
}