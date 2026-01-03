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
> where
    Self: Model,
{
    fn get_state(&mut self) -> StateType;

    fn get_state_async(&mut self, _: (), _: &mut Context<Self>) -> impl Future<Output = StateType> + Send {
        async move { self.get_state() }
    }
    fn get_next_event_meta(&mut self) -> EventMetadata;
    fn previous_state(&mut self) -> &mut Option<StateType>;
    fn resources(&mut self) -> &mut VecDequeStock<ResourceType>;
    fn state_emitter(&mut self) -> &mut Output<EventMetadata>;

    fn add_one(&mut self, payload: (Option<ResourceType>, EventMetadata), cx: &mut Context<Self>) -> impl Future<Output = ()> + Send {
        async move {
            *self.previous_state() = Some(self.get_state().clone());
            if let (Some(resource), _) = payload.clone() {
                self.resources().add_one(resource);
            }
            let total = self.resources().total();
            let event_id = self.log_type_add_one(&mut payload.1.clone(), total, payload.0.clone(), cx).await;
            let prev = self.previous_state().clone();
            let cur = self.get_state().clone();
            if prev.is_none() || !prev.as_ref().unwrap().is_same_state(&cur) {
                let t1ns = cx.time() + Duration::from_nanos(1);
                cx.schedule_event(t1ns, Self::emit_change, (cur.clone(), event_id)).unwrap();
            }
            *self.previous_state() = Some(cur);
        }
    }

    fn add_multi(&mut self, payload: (Vec<ResourceType>, EventMetadata), cx: &mut Context<Self>) -> impl Future<Output = ()> {
        async move {
            *self.previous_state() = Some(self.get_state().clone());
            self.resources().add_multi(payload.0.clone());
            let total = self.resources().total();
            let event_id = self.log_type_add_multi(&mut payload.1.clone(), total, payload.0.clone(), cx).await;
            let prev = self.previous_state().clone();
            let cur = self.get_state().clone();
            if prev.is_none() || !prev.as_ref().unwrap().is_same_state(&cur) {
                let t1ns = cx.time() + Duration::from_nanos(1);
                cx.schedule_event(t1ns, Self::emit_change, (cur.clone(), event_id)).unwrap();
            }
            *self.previous_state() = Some(cur);
        }
    }

    fn remove_one(&mut self, payload: EventMetadata, cx: &mut Context<Self>) -> impl Future<Output = Option<ResourceType>> + Send 
    where ResourceType:
    {
        async move {
            *self.previous_state() = Some(self.get_state().clone());
            let removed_entry = self.resources().remove_one();
            let total = self.resources().total();
            let event_id = self.log_type_remove_one(&mut payload.clone(), total, removed_entry.clone(), cx).await;
            let prev = self.previous_state().clone();
            let cur = self.get_state().clone();
            if prev.is_none() || !prev.as_ref().unwrap().is_same_state(&cur) {
                let t1ns = cx.time() + Duration::from_nanos(1);
                cx.schedule_event(t1ns, Self::emit_change, (cur.clone(), event_id)).unwrap();
            }
            *self.previous_state() = Some(cur);
            removed_entry
        }
    }

    fn remove_multi(&mut self, payload: (usize, EventMetadata), cx: &mut Context<Self>) -> impl Future<Output = Vec<ResourceType>> + Send 
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
            let event_id = self.log_type_remove_multi(&mut payload.1.clone(), total, removed_entries.clone(), cx).await;
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

    fn emit_change(&mut self, payload: (StateType, EventMetadata), cx: &mut Context<Self>) -> impl Future<Output = ()> + Send {
        async move {
            let event_id = self.log_type_state_change(&mut payload.1.clone(), payload.0.clone(), cx).await;
            self.state_emitter().send(event_id).await;
        }
    }
    fn log_type_add_one(&mut self, source_event: &mut EventMetadata, balance: usize, resource: Option<ResourceType>, cx: &mut Context<Self>) -> impl Future<Output = EventMetadata> + Send;
    fn log_type_add_multi(&mut self, source_event: &mut EventMetadata, balance: usize, resource: Vec<ResourceType>, cx: &mut Context<Self>) -> impl Future<Output = EventMetadata> + Send;
    fn log_type_remove_one(&mut self, source_event: &mut EventMetadata, balance: usize, resource: Option<ResourceType>, cx: &mut Context<Self>) -> impl Future<Output = EventMetadata> + Send;
    fn log_type_remove_multi(&mut self, source_event: &mut EventMetadata, balance: usize, resource: Vec<ResourceType>, cx: &mut Context<Self>) -> impl Future<Output = EventMetadata> + Send;
    fn log_type_state_change(&mut self, source_event: &mut EventMetadata, new_state: StateType, cx: &mut Context<Self>) -> impl Future<Output = EventMetadata> + Send;
}

#[derive(Debug, Clone, Serialize)]
pub struct DiscProcessLog<ItemType>
where
    ItemType: Clone + Serialize + Debug,
{
    pub time: String,
    pub event: EventMetadata,
    pub source_event: EventMetadata,
    pub element_name: String,
    pub element_type: String,
    pub details: DefaultDiscProcessLogType<ItemType>,
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
        &mut self, source_event: &mut EventMetadata,
        cx: &mut Context<Self>,
        duration_since_prev: Duration
    ) -> impl Future<Output = ()>;

    fn update_state_since_last_update(
        &mut self,
        source_event: &mut EventMetadata,
        cx: &mut Context<Self>,
    ) -> impl Future<Output = ()> {
        async move {
            if let Some((scheduled_time, _)) = self.scheduled_event() {
                if *scheduled_time <= cx.time() {
                    *self.scheduled_event() = None;
                }
            }

            let duration_since_prev = cx.time().duration_since(*self.previous_check_time());

            self.update_process_state_since_prev_event(source_event, cx, duration_since_prev).await;
        }
    }

    fn log_type_process_success(&mut self, source_event: &mut EventMetadata, quantity: usize, resources: Vec<ItemType>, cx: &mut Context<Self>) -> impl Future<Output = EventMetadata>;

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
        source_event: &mut EventMetadata,
        cx: &mut Context<Self>,
    ) -> impl Future<Output = ()>;

    fn log_type_withdraw_request(&mut self, source_event: &mut EventMetadata, quantity: usize, cx: &mut Context<Self>) -> impl Future<Output = EventMetadata>;
    fn log_type_process_start(&mut self, source_event: &mut EventMetadata, quantity: usize, resources: Vec<ItemType>, cx: &mut Context<Self>) -> impl Future<Output = EventMetadata>;
    fn log_type_process_failure(&mut self, source_event: &mut EventMetadata, reason: &'static str, cx: &mut Context<Self>) -> impl Future<Output = EventMetadata>;

}

pub trait DiscProcessUpdateForNextEvent<
    ItemType,
    ProcessLogType,
>
where 
Self: DiscProcessCore<
    ItemType,
    ProcessLogType,
>,
ItemType: Clone + Debug + Serialize + Send + 'static,
ProcessLogType: Clone + Debug + Serialize + Send + 'static,
{
    fn update_state_for_next_event(
        &mut self,
        source_event: &mut EventMetadata,
        cx: &mut Context<Self>,
    ) -> impl Future<Output = ()> + Send {
        async move {
            let next_event = self.time_to_next_process_event();

            if let Some(time_until_next) = next_event {
                if time_until_next.is_zero() {
                    panic!("Time until next event is zero!");
                }

                let next_time = cx.time() + *time_until_next;

                if let Some((scheduled_time, action_key)) =
                    self.scheduled_event().take()
                {
                    if next_time < scheduled_time {
                        action_key.cancel();
                        let new_key = cx
                            .schedule_keyed_event(
                                next_time,
                                Self::update_state,
                                source_event.clone(),
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
                            source_event.clone(),
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
    fn get_next_event_meta(&mut self) -> EventMetadata;
    fn scheduled_event(&mut self) -> &mut Option<(MonotonicTime, ActionKey)>;
    fn previous_check_time(&mut self) -> &mut MonotonicTime;
    fn time_to_next_process_event(&mut self) -> &mut Option<Duration>;

    fn update_state(
        &mut self,
        source_event: EventMetadata,
        cx: &mut Context<Self>,
    ) -> impl Future<Output = ()> + Send;
}