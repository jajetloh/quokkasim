use serde::Serialize;
use std::{collections::{HashMap, HashSet, VecDeque}, fmt::Debug, ops::{Deref, DerefMut}, time::Duration};
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
pub struct DiscProcessLog<DetailsType>
where
    DetailsType: Clone + Serialize,
{
    pub time: String,
    pub event_id: EventId,
    pub source_event_id: EventId,
    pub element_name: String,
    pub element_type: String,
    pub details: DetailsType,
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

pub trait Emptyable {
    fn is_empty(&self) -> bool;
}
impl<T> Emptyable for Option<T> {
    fn is_empty(&self) -> bool {
        self.is_none()
    }
}
impl<T> Emptyable for Vec<T> {
    fn is_empty(&self) -> bool {
        self.is_empty()
    }
}
impl<T> Emptyable for VecDeque<T> {
    fn is_empty(&self) -> bool {
        self.is_empty()
    }
}
impl<T, U> Emptyable for HashMap<T, U> {
    fn is_empty(&self) -> bool {
        self.is_empty()
    }
}
impl<T> Emptyable for HashSet<T> {
    fn is_empty(&self) -> bool {
        self.is_empty()
    }
}

pub trait DiscProcessUpdateSinceLast<
    ItemType,
    ProcessLogType,
>: DiscProcessCore<
    ItemType,
    ProcessLogType,
>
where
    ItemType: Clone + Debug + Serialize + Send + 'static,
    ProcessLogType: Clone + Debug + Serialize + Send + 'static,
{
    fn update_process_state_since_prev_event(
        &mut self, source_event_id: &mut EventId,
        cx: &mut Context<Self>,
        duration_since_prev: Duration
    ) -> impl Future<Output = ()>;

    fn update_state_since_last_update(
        &mut self,
        source_event_id: &mut EventId,
        mut cx: &mut Context<Self>,
    ) -> impl Future<Output = ()> {
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
                self.process_state().is_empty() && !is_in_delay && !is_env_blocked;

            if !is_in_delay && !is_env_blocked {
                self.update_process_state_since_prev_event(source_event_id, cx, duration_since_prev).await;
            }

            if !is_env_blocked && (is_in_delay || is_in_process) {
                let transition = self.delay_modes().update_state(duration_since_prev);
                if transition.has_changed() {
                    if let Some(delay_name) = &transition.from {
                        *source_event_id = self.log_type_delay_end(source_event_id, delay_name.clone(), &mut cx).await;
                    }
                    if let Some(delay_name) = &transition.to {
                        *source_event_id = self.log_type_delay_start(source_event_id, delay_name.clone(), &mut cx).await;
                    }
                }
            }
        }
    }
}

pub trait DiscProcessUpdateDecisionLogic<
    ItemType,
    ProcessLogType,
>: DiscProcessCore<
    ItemType,
    ProcessLogType,
> where 
    ItemType: Clone + Debug + Serialize + Send + 'static,
    ProcessLogType: Clone + Debug + Serialize + Send + 'static,
{
    fn update_state_decision_logic(
        &mut self,
        source_event_id: &mut EventId,
        cx: &mut Context<Self>,
    ) -> impl Future<Output = ()>;
}

pub trait DiscProcessUpdateForNextEvent<
    ItemType,
    ProcessLogType,
>: DiscProcessCore<
    ItemType,
    ProcessLogType,
> where 
    ItemType: Clone + Debug + Serialize + Send + 'static,
    ProcessLogType: Clone + Debug + Serialize + Send + 'static,
{
    fn update_state_for_next_event(
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
                *self.time_to_next_delay_event(),
                *self.time_to_next_process_event(),
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

pub trait DiscProcessCore<
    ItemType,
    LogRecordType,
>: Model
where
    ItemType: Clone + Debug + Serialize + Send + 'static,
    LogRecordType: Clone + Send + 'static,
{
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

    fn log_type_withdraw_request(&mut self, source_event_id: &mut EventId, quantity: usize, cx: &mut Context<Self>) -> impl Future<Output = EventId>;
    fn log_type_process_start(&mut self, source_event_id: &mut EventId, quantity: usize, resources: Vec<ItemType>, cx: &mut Context<Self>) -> impl Future<Output = EventId>;
    fn log_type_process_success(&mut self, source_event_id: &mut EventId, quantity: usize, resources: Vec<ItemType>, cx: &mut Context<Self>) -> impl Future<Output = EventId>;
    fn log_type_process_failure(&mut self, source_event_id: &mut EventId, reason: &'static str, cx: &mut Context<Self>) -> impl Future<Output = EventId>;
    fn log_type_process_stopped(&mut self, source_event_id: &mut EventId, reason: &'static str, cx: &mut Context<Self>) -> impl Future<Output = EventId>;
    fn log_type_process_continue(&mut self, source_event_id: &mut EventId, reason: &'static str, cx: &mut Context<Self>) -> impl Future<Output = EventId>;
    fn log_type_delay_start(&mut self, source_event_id: &mut EventId, delay_name: String, cx: &mut Context<Self>) -> impl Future<Output = EventId> + Send;
    fn log_type_delay_end(&mut self, source_event_id: &mut EventId, delay_name: String, cx: &mut Context<Self>) -> impl Future<Output = EventId> + Send;
    fn log_type_state_change(&mut self, source_event_id: &mut EventId, new_state: DiscStockState, cx: &mut Context<Self>) -> impl Future<Output = EventId> + Send;

    fn update_state(
        &mut self,
        source_event_id: EventId,
        cx: &mut Context<Self>,
    ) -> impl Future<Output = ()> + Send;
}