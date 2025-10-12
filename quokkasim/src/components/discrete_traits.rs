use serde::Serialize;
use std::{collections::VecDeque, fmt::Debug, ops::{Deref, DerefMut}, time::Duration};
use crate::prelude::*;

pub trait Generator<T> {
    fn next(&mut self) -> T;
}

/// Public stock-state enum for discrete resources.
#[derive(Debug, Clone, Serialize)]
pub enum DiscStockState {
    Normal { occupied: usize, empty: usize },
    Full { occupied: usize, empty: usize },
    Empty { occupied: usize, empty: usize },
}

impl StockState for DiscStockState {
    fn is_same_state(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (DiscStockState::Empty { .. }, DiscStockState::Empty { .. })
                | (DiscStockState::Normal { .. }, DiscStockState::Normal { .. })
                | (DiscStockState::Full { .. }, DiscStockState::Full { .. })
        )
    }
}

pub struct VecDequeStock<T> {
    inner: VecDeque<T>,
    access: VecDequeAccess,
}

impl<T> Deref for VecDequeStock<T> {
    type Target = VecDeque<T>;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
impl<T> DerefMut for VecDequeStock<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl<T> VecDequeStock<T> {
    pub fn new(access: VecDequeAccess) -> Self {
        Self {
            inner: VecDeque::new(),
            access,
        }
    }
}

pub enum VecDequeAccess {
    FIFO,
    LIFO,
}

impl<T> DiscreteArithmetic<T> for VecDequeStock<T> {
    fn add_one(&mut self, arg: T) {
        match self.access {
            VecDequeAccess::FIFO => self.push_back(arg),
            VecDequeAccess::LIFO => self.push_front(arg),
        }
    }
    fn add_multi(&mut self, arg: Vec<T>) {
        for a in arg {
            match self.access {
                VecDequeAccess::FIFO => self.push_back(a),
                VecDequeAccess::LIFO => self.push_front(a),
            }
        }
    }
    fn remove_one(&mut self) -> Option<T> {
        match self.access {
            VecDequeAccess::FIFO => self.pop_front(),
            VecDequeAccess::LIFO => self.pop_back(),
        }
    }
    fn remove_multi(&mut self, count: usize) -> Vec<T> {
        let mut removed = Vec::new();
        for _ in 0..count {
            if let Some(item) = match self.access {
                VecDequeAccess::FIFO => self.pop_front(),
                VecDequeAccess::LIFO => self.pop_back(),
            } {
                removed.push(item);
            } else {
                break;
            }
        }
        removed
    }
    fn remove_all(&mut self) -> Vec<T> {
        let mut removed = Vec::new();
        while let Some(item) = match self.access {
            VecDequeAccess::FIFO => self.pop_front(),
            VecDequeAccess::LIFO => self.pop_back(),
        } {
            removed.push(item);
        }
        removed
    }
    fn total(&self) -> usize {
        self.len()
    }
}

/// Arithmetic for “discrete” resources (counts).
pub trait DiscreteArithmetic<T> {
    fn add_one(&mut self, arg: T);
    fn add_multi(&mut self, arg: Vec<T>);
    fn remove_one(&mut self) -> Option<T>;
    fn remove_multi(&mut self, count: usize) -> Vec<T>;
    fn remove_all(&mut self) -> Vec<T>;
    fn total(&self) -> usize;
}

pub trait DiscStock<
    ResourceType: Clone + Send + 'static,
    StateType: StockState + Clone + Send + 'static,
    LogRecordType: Clone + Send + 'static,
    LogDetailsType: Clone + Send + 'static,
> where
    Self: Model,
    Self: ToLogRecord<LogDetailsType, LogRecordType>,
{
    fn get_state(&mut self) -> StateType;

    fn get_state_async(&mut self, _: (), _: &mut Context<Self>) -> impl Future<Output = StateType> + Send {
        async move { self.get_state() }
    }

    fn log_emitter(&mut self) -> &mut Output<LogRecordType>;
    fn get_next_event_id(&mut self) -> EventId;
    fn previous_state(&mut self) -> &mut Option<StateType>;
    fn resources(&mut self) -> &mut VecDequeStock<ResourceType>;
    fn state_emitter(&mut self) -> &mut Output<EventId>;

    fn add_one(&mut self, payload: (Option<ResourceType>, EventId), cx: &mut Context<Self>) -> impl Future<Output = ()> + Send {
        async move {
            *self.previous_state() = Some(self.get_state().clone());
            if let (Some(resource), _) = payload.clone() {
                self.resources().add_one(resource);
            }
            let total = self.resources().total();
            let event_id = self
                .log(cx.time(), payload.1.clone(), self.log_type_add_one(total, payload.0.clone()))
                .await;
            let prev = self.previous_state().clone();
            let cur = self.get_state().clone();
            if prev.is_none() || !prev.as_ref().unwrap().is_same_state(&cur) {
                let t1ns = cx.time() + Duration::from_nanos(1);
                cx.schedule_event(t1ns, Self::emit_change, (cur.clone(), event_id)).unwrap();
            }
            *self.previous_state() = Some(cur);
        }
    }

    fn add_multi(&mut self, payload: (Vec<ResourceType>, EventId), cx: &mut Context<Self>) -> impl Future<Output = ()> + Send {
        async move {
            *self.previous_state() = Some(self.get_state().clone());
            self.resources().add_multi(payload.0.clone());
            let total = self.resources().total();
            let event_id = self
                .log(cx.time(), payload.1.clone(), self.log_type_add_multi(total, payload.0.clone()))
                .await;
            let prev = self.previous_state().clone();
            let cur = self.get_state().clone();
            if prev.is_none() || !prev.as_ref().unwrap().is_same_state(&cur) {
                let t1ns = cx.time() + Duration::from_nanos(1);
                cx.schedule_event(t1ns, Self::emit_change, (cur.clone(), event_id)).unwrap();
            }
            *self.previous_state() = Some(cur);
        }
    }

    fn remove_multi(&mut self, payload: (usize, EventId), cx: &mut Context<Self>) -> impl Future<Output = Vec<ResourceType>> + Send 
    where ResourceType:
    {
        async move {
            *self.previous_state() = Some(self.get_state().clone());
            let mut removed_entries = Vec::new();
            for _ in 0..payload.0 {
                if let Some(removed) = self.resources().remove_one() {
                    removed_entries.push(removed);
                } else {
                    break;
                }
            }
            let total = self.resources().total();
            let event_id = self
                .log(cx.time(), payload.1.clone(), self.log_type_remove_multi(total, removed_entries.clone()))
                .await;
            let prev = self.previous_state().clone();
            let cur = self.get_state().clone();
            if prev.is_none() || !prev.as_ref().unwrap().is_same_state(&cur) {
                let t1ns = cx.time() + Duration::from_nanos(1);
                cx.schedule_event(t1ns, Self::emit_change, (cur.clone(), event_id)).unwrap();
            }
            *self.previous_state() = Some(cur);
            removed_entries
        }
    }

    fn emit_change(&mut self, payload: (StateType, EventId), cx: &mut Context<Self>) -> impl Future<Output = ()> + Send {
        async move {
            let nm = self
                .log(cx.time(), payload.1, self.log_type_state_change(payload.0.clone()))
                .await;
            self.state_emitter().send(nm).await;
        }
    }

    fn log<D: Into<LogDetailsType> + Send>(
        &mut self, now: MonotonicTime, source_event: EventId, details: D
    ) -> impl Future<Output = EventId> + Send {
        async move {
            let eid = self.get_next_event_id();
            let record = self.to_record(now, source_event.clone(), eid.clone(), details.into());
            self.log_emitter().send(record.clone()).await;
            eid
        }
    }

    fn log_type_add_one(&self, balance: usize, resource: Option<ResourceType>) -> LogDetailsType;
    fn log_type_add_multi(&self, balance: usize, resource: Vec<ResourceType>) -> LogDetailsType;
    fn log_type_remove_one(&self, balance: usize, resource: Option<ResourceType>) -> LogDetailsType;
    fn log_type_remove_multi(&self, balance: usize, resource: Vec<ResourceType>) -> LogDetailsType;
    fn log_type_state_change(&self, new_state: StateType) -> LogDetailsType;
}

#[derive(Debug, Clone, Serialize)]
pub struct DiscProcessLog<DetailsType, ItemType>
where
    DetailsType: Clone + Serialize,
    ItemType: Clone + Debug + Serialize,
{
    pub time: String,
    pub event_id: EventId,
    pub source_event_id: EventId,
    pub element_name: String,
    pub element_type: String,
    pub details: DetailsType,
    #[serde(skip)]
    pub phantom: std::marker::PhantomData<ItemType>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event_type")]
pub enum DefaultDiscProcessLogType<ItemType>
where
    ItemType: Clone + Debug + Serialize,
{
    WithdrawRequest { quantity: usize },
    ProcessStart { quantity: usize, resources: Vec<ItemType> },
    ProcessSuccess { quantity: usize, resources: Vec<ItemType> },
    ProcessFailure { reason: &'static str },
    ProcessStopped { reason: &'static str },
    ProcessContinue { reason: &'static str },
    DelayStart { delay_name: String },
    DelayEnd { delay_name: String },
    StateChange { new_state: DiscStockState },
}


pub trait DiscProcessCore<
    ItemType,
    LogRecordType,
    LogDetailsType,
>: Model + ToLogRecord<LogDetailsType, LogRecordType>
where
    ItemType: Clone + Debug + Serialize + Send + 'static,
    LogRecordType: Clone + Send + 'static,
    LogDetailsType: Clone + Send + 'static,
{
    fn log<D: Into<LogDetailsType> + Send>(
        &mut self,
        now: MonotonicTime,
        source_event_id: EventId,
        details: D,
    ) -> impl Future<Output = EventId> + Send {
        async move {
            let event_id = self.get_next_event_id();
            let record = self.to_record(
                now,
                source_event_id.clone(),
                event_id.clone(),
                details.into(),
            );
            self.log_emitter().send(record).await;
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
    fn process_state(&mut self) -> &mut Option<(Duration, Vec<ItemType>)>;
    fn env_state(&mut self) -> &mut BasicEnvironmentState;
    fn req_environment(&mut self) -> &mut Requestor<(), BasicEnvironmentState>;
    fn time_to_next_process_event(&mut self) -> &mut Option<Duration>;
    fn time_to_next_delay_event(&mut self) -> &mut Option<Duration>;

    fn log_type_withdraw_request(&self, quantity: usize) -> LogDetailsType;
    fn log_type_process_start(
        &self,
        quantity: usize,
        resources: Vec<ItemType>,
    ) -> LogDetailsType;
    fn log_type_process_success(
        &self,
        quantity: usize,
        resources: Vec<ItemType>,
    ) -> LogDetailsType;
    fn log_type_process_failure(&self, reason: &'static str) -> LogDetailsType;
    fn log_type_process_stopped(&self, reason: &'static str) -> LogDetailsType;
    fn log_type_process_continue(&self, reason: &'static str) -> LogDetailsType;
    fn log_type_delay_start(&self, delay_name: String) -> LogDetailsType;
    fn log_type_delay_end(&self, delay_name: String) -> LogDetailsType;
    fn log_type_state_change(&self, new_state: DiscStockState) -> LogDetailsType;
}

pub trait DiscProcess<
    ItemType,
    LogRecordType,
    LogDetailsType,
>: DiscProcessCore<ItemType, LogRecordType, LogDetailsType>
where
    ItemType: Clone + Debug + Serialize + Send + 'static,
    LogRecordType: Clone + Send + 'static,
    LogDetailsType: Clone + Send + 'static,
{
    fn req_upstream(&mut self) -> &mut Requestor<(), DiscStockState>;
    fn withdraw_upstream(&mut self)
        -> &mut Requestor<(usize, EventId), Vec<ItemType>>;
    fn req_downstream(&mut self) -> &mut Requestor<(), DiscStockState>;
    fn push_downstream(&mut self)
        -> &mut Output<(Vec<ItemType>, EventId)>;
    fn process_quantity_distr(&mut self) -> &mut Distribution;
    fn process_time_distr(&mut self) -> &mut Distribution;

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
            if let Some((scheduled_time, _)) = self.scheduled_event() {
                if *scheduled_time <= cx.time() {
                    *self.scheduled_event() = None;
                }
            }

            let duration_since_prev =
                cx.time().duration_since(*self.previous_check_time());

            let is_in_delay = self.delay_modes().active_delay().is_some();
            let is_env_blocked =
                matches!(self.env_state(), BasicEnvironmentState::Stopped);
            let is_in_process =
                self.process_state().is_some() && !is_in_delay && !is_env_blocked;

            if !is_in_delay && !is_env_blocked {
                if let Some((mut time_left, resources)) = self.process_state().take() {
                    time_left = time_left.saturating_sub(duration_since_prev);
                    if time_left.is_zero() {
                        let quantity = resources.len();
                        let log_payload = resources.clone();
                        *source_event_id = self
                            .log(
                                cx.time(),
                                source_event_id.clone(),
                                self.log_type_process_success(quantity, log_payload),
                            )
                            .await;
                        self.push_downstream()
                            .send((resources, source_event_id.clone()))
                            .await;
                        *self.time_to_next_process_event() = None;
                    } else {
                        *self.process_state() = Some((time_left, resources));
                        *self.time_to_next_process_event() = Some(time_left);
                    }
                }
            }

            if !is_env_blocked && (is_in_delay || is_in_process) {
                let transition = self.delay_modes().update_state(duration_since_prev);
                if transition.has_changed() {
                    if let Some(delay_name) = &transition.from {
                        *source_event_id = self
                            .log(
                                cx.time(),
                                source_event_id.clone(),
                                self.log_type_delay_end(delay_name.clone()),
                            )
                            .await;
                    }
                    if let Some(delay_name) = &transition.to {
                        *source_event_id = self
                            .log(
                                cx.time(),
                                source_event_id.clone(),
                                self.log_type_delay_start(delay_name.clone()),
                            )
                            .await;
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
            let time_now = cx.time();

            let new_env_state = match self.req_environment().send(()).await.next() {
                Some(env) => env,
                None => BasicEnvironmentState::Normal,
            };

            match (&self.env_state(), &new_env_state) {
                (BasicEnvironmentState::Normal, BasicEnvironmentState::Stopped) => {
                    *source_event_id = self
                        .log(
                            time_now,
                            source_event_id.clone(),
                            self.log_type_process_stopped("Stopped by environment"),
                        )
                        .await;
                    *self.env_state() = BasicEnvironmentState::Stopped;
                }
                (BasicEnvironmentState::Stopped, BasicEnvironmentState::Normal) => {
                    *source_event_id = self
                        .log(
                            time_now,
                            source_event_id.clone(),
                            self.log_type_process_continue("Resumed by environment"),
                        )
                        .await;
                    *self.env_state() = BasicEnvironmentState::Normal;
                }
                _ => {}
            }

            let is_env_stopped =
                matches!(self.env_state(), BasicEnvironmentState::Stopped);
            let has_active_delay =
                self.delay_modes().active_delay().is_some() || is_env_stopped;

            match (self.process_state(), has_active_delay) {
                (None, false) => {
                    let upstream_state = self.req_upstream().send(()).await.next();
                    let downstream_state =
                        self.req_downstream().send(()).await.next();

                    match (&upstream_state, &downstream_state) {
                        (
                            Some(DiscStockState::Normal { .. })
                            | Some(DiscStockState::Full { .. }),
                            Some(DiscStockState::Empty { .. })
                            | Some(DiscStockState::Normal { .. }),
                        ) => {
                            let mut requested =
                                self.process_quantity_distr().sample().round();
                            if !requested.is_finite() {
                                requested = 1.0;
                            }
                            let requested = requested.clamp(1.0, u32::MAX as f64) as usize;

                            *source_event_id = self
                                .log(
                                    time_now,
                                    source_event_id.clone(),
                                    self.log_type_withdraw_request(requested),
                                )
                                .await;

                            let pulled = self
                                .withdraw_upstream()
                                .send((requested, source_event_id.clone()))
                                .await
                                .next();

                            match pulled {
                                Some(mut batch) => {
                                    if batch.is_empty() {
                                        *source_event_id = self
                                            .log(
                                                time_now,
                                                source_event_id.clone(),
                                                self.log_type_process_failure(
                                                    "Upstream returned no items",
                                                ),
                                            )
                                            .await;
                                        *self.time_to_next_process_event() = None;
                                        return;
                                    }

                                    let mut process_secs =
                                        self.process_time_distr().sample();
                                    if !process_secs.is_finite() {
                                        process_secs = 0.0;
                                    }
                                    process_secs = process_secs.max(0.0);
                                    let mut process_duration =
                                        Duration::from_secs_f64(process_secs);
                                    if process_duration.is_zero() {
                                        process_duration = Duration::from_nanos(1);
                                    }

                                    let quantity = batch.len();
                                    let log_payload = batch.clone();

                                    *self.process_state() =
                                        Some((process_duration, batch));
                                    *source_event_id = self
                                        .log(
                                            time_now,
                                            source_event_id.clone(),
                                            self.log_type_process_start(
                                                quantity,
                                                log_payload,
                                            ),
                                        )
                                        .await;
                                    *self.time_to_next_process_event() =
                                        Some(process_duration);
                                }
                                None => {
                                    *source_event_id = self
                                        .log(
                                            time_now,
                                            source_event_id.clone(),
                                            self.log_type_process_failure(
                                                "Upstream requestor closed",
                                            ),
                                        )
                                        .await;
                                    *self.time_to_next_process_event() = None;
                                }
                            }
                        }
                        (Some(DiscStockState::Empty { .. }), _) => {
                            *source_event_id = self
                                .log(
                                    time_now,
                                    source_event_id.clone(),
                                    self.log_type_process_failure("Upstream is empty"),
                                )
                                .await;
                            *self.time_to_next_process_event() = None;
                        }
                        (None, _) => {
                            *source_event_id = self
                                .log(
                                    time_now,
                                    source_event_id.clone(),
                                    self.log_type_process_failure(
                                        "Upstream is not connected",
                                    ),
                                )
                                .await;
                            *self.time_to_next_process_event() = None;
                        }
                        (_, None) => {
                            *source_event_id = self
                                .log(
                                    time_now,
                                    source_event_id.clone(),
                                    self.log_type_process_failure(
                                        "Downstream is not connected",
                                    ),
                                )
                                .await;
                            *self.time_to_next_process_event() = None;
                        }
                        (_, Some(DiscStockState::Full { .. })) => {
                            *source_event_id = self
                                .log(
                                    time_now,
                                    source_event_id.clone(),
                                    self.log_type_process_failure("Downstream is full"),
                                )
                                .await;
                            *self.time_to_next_process_event() = None;
                        }
                    }
                }
                (Some((time_left, _)), false) => {
                    *self.time_to_next_process_event() = Some(*time_left);
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
            let is_env_stopped =
                matches!(self.env_state(), BasicEnvironmentState::Stopped);
            let has_active_delay =
                self.delay_modes().active_delay().is_some() || is_env_stopped;

            if self.process_state().is_some()
                || has_active_delay
                || !is_env_stopped
            {
                *self.time_to_next_delay_event() = self
                    .delay_modes()
                    .get_next_event()
                    .map(|(_, delay_state)| delay_state.as_duration());
            } else {
                *self.time_to_next_delay_event() = None;
            }

            let next_event = [
                self.time_to_next_delay_event().clone(),
                self.time_to_next_process_event().clone(),
            ]
            .into_iter()
            .flatten()
            .min();

            if let Some(time_until_next) = next_event {
                if time_until_next.is_zero() {
                    panic!("Time until next event is zero!");
                }

                let next_time = cx.time() + time_until_next;

                if let Some((scheduled_time, action_key)) =
                    self.scheduled_event().take()
                {
                    if next_time < scheduled_time {
                        action_key.cancel();
                        let new_key = cx
                            .schedule_keyed_event(
                                next_time,
                                Self::update_state,
                                source_event_id.clone(),
                            )
                            .unwrap();
                        *self.scheduled_event() = Some((next_time, new_key));
                    } else {
                        *self.scheduled_event() =
                            Some((scheduled_time, action_key));
                    }
                } else {
                    let new_key = cx
                        .schedule_keyed_event(
                            next_time,
                            Self::update_state,
                            source_event_id.clone(),
                        )
                        .unwrap();
                    *self.scheduled_event() = Some((next_time, new_key));
                }
            }

            *self.previous_check_time() = cx.time();
        }
    }
}