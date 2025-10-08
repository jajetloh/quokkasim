use serde::Serialize;
use std::{collections::VecDeque, fmt::Debug, ops::{Deref, DerefMut}, time::Duration};
use crate::prelude::*;

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
    fn remove_multi(&mut self, count: u32) -> Vec<T> {
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
    fn total(&self) -> u32 {
        self.len() as u32
    }
}

/// Arithmetic for “discrete” resources (counts).
pub trait DiscreteArithmetic<T> {
    fn add_one(&mut self, arg: T);
    fn add_multi(&mut self, arg: Vec<T>);
    fn remove_one(&mut self) -> Option<T>;
    fn remove_multi(&mut self, count: u32) -> Vec<T>;
    fn remove_all(&mut self) -> Vec<T>;
    fn total(&self) -> u32;
}

/// A discrete “resource” is anything you can do those ops on + send/clone/debug/serialize…
pub trait DiscreteResource<T>: 
    DiscreteArithmetic<T> +
    Clone +
    Send +
    Debug +
    Default +
    Serialize
{}
impl<T> DiscreteResource<T> for T 
where T: DiscreteArithmetic<T> + Clone + Send + Debug + Default + Serialize
{}

pub trait DiscreteStock<
    ResourceType: DiscreteResource<u32> + 'static,
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

    fn add_one(&mut self, payload: (ResourceType, EventId), cx: &mut Context<Self>) -> impl Future<Output = ()> + Send {
        async move {
            *self.previous_state() = Some(self.get_state().clone());
            self.resources().add_one(payload.0.clone());
            let total = self.resources().total();
            let event_id = self
                .log(cx.time(), payload.1.clone(), self.log_type_add(total, payload.0.clone()))
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

    fn remove<T>(&mut self, payload: (T, EventId), cx: &mut Context<Self>) -> impl Future<Output = Option<ResourceType>> + Send 
    where ResourceType:
    {
        async move {
            *self.previous_state() = Some(self.get_state().clone());
            let removed = self.resources().remove_one();
            let total = self.resources().total();
            let event_id = self
                .log(cx.time(), payload.1.clone(), self.log_type_remove(total, removed.clone()))
                .await;
            let prev = self.previous_state().clone();
            let cur = self.get_state().clone();
            if prev.is_none() || !prev.as_ref().unwrap().is_same_state(&cur) {
                let t1ns = cx.time() + Duration::from_nanos(1);
                cx.schedule_event(t1ns, Self::emit_change, (cur.clone(), event_id)).unwrap();
            }
            *self.previous_state() = Some(cur);
            removed
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

    fn log_type_add(&self, balance: u32, resource: ResourceType) -> LogDetailsType;
    fn log_type_remove(&self, balance: u32, resource: Option<ResourceType>) -> LogDetailsType;
    fn log_type_state_change(&self, new_state: StateType) -> LogDetailsType;
}
