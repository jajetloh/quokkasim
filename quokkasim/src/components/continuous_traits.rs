use serde::Serialize;
use std::{fmt::Debug, time::Duration};

use crate::{
    common::{EventMetadata, StockState},
    nexosim::{ActionKey, Context, Model, MonotonicTime, Output},
};

pub trait ContArithmetic {
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

impl ContArithmetic for f64 {
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

impl<const N: usize> ContArithmetic for [f64; N] {
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

pub trait ContResource: ContArithmetic + Clone + Send + Debug + Default + Serialize {}
impl<T> ContResource for T where T: ContArithmetic + Clone + Send + Debug + Default + Serialize {}

pub trait ContProcessUpdateSinceLast<
    ResourceType: ContResource + 'static,
    LogRecordType: Clone + Send + 'static,
>: ContProcessCore<ResourceType, LogRecordType>
{
    // Resolves the state of this process from the previous update time to now.
    // Handles some edge case handling and logging as well.
    fn update_state_since_last_update(
        &mut self,
        source_event_id: &mut EventMetadata,
        cx: &mut Context<Self>,
    ) -> impl Future<Output = ()> {
        async move {
            if let Some((scheduled_time, _)) = self.scheduled_event() {
                if *scheduled_time <= cx.time() {
                    *self.scheduled_event() = None;
                }
            }

            let duration_since_prev = cx.time().duration_since(*self.previous_check_time());

            self.update_process_state_since_prev_event(source_event_id, cx, duration_since_prev).await;
        }
    }

    // Concrete function to update the internal process state from the previous time to now
    fn update_process_state_since_prev_event(
        &mut self, source_event_id: &mut EventMetadata,
        cx: &mut Context<Self>,
        duration_since_prev: Duration
    ) -> impl Future<Output = ()>;

    fn log_type_process_success(&mut self, source_event_id: &mut EventMetadata, quantity: f64, resource: ResourceType, cx: &mut Context<Self>) -> impl Future<Output = EventMetadata>;
}

pub trait ContProcessUpdateDecisionLogic<
    ResourceType: ContResource + 'static,
    LogRecordType: Clone + Send + 'static,  
>: ContProcessCore<ResourceType, LogRecordType>
{
    fn update_state_decision_logic(
        &mut self,
        source_event_id: &mut EventMetadata,
        cx: &mut Context<Self>,
    ) -> impl Future<Output = ()>;

    fn log_type_withdraw_request(&mut self, source_event_id: &mut EventMetadata, quantity: f64, cx: &mut Context<Self>) -> impl Future<Output = EventMetadata>;
    fn log_type_process_start(&mut self, source_event_id: &mut EventMetadata, quantity: f64, resource: ResourceType, cx: &mut Context<Self>) -> impl Future<Output = EventMetadata>;
    fn log_type_process_failure(&mut self, source_event_id: &mut EventMetadata, reason: &'static str, cx: &mut Context<Self>) -> impl Future<Output = EventMetadata>;

}

pub trait ContProcessUpdateForNextEvent<
    ResourceType: ContResource + 'static,
    LogRecordType: Clone + Send + 'static,
>: ContProcessCore<ResourceType, LogRecordType>
{
    fn update_state_for_next_event(
        &mut self,
        source_event_id: &mut EventMetadata,
        cx: &mut Context<Self>,
    ) -> impl Future<Output = ()> {
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

pub trait ContProcessCore<
    ResourceType: ContResource + 'static,
    LogRecordType: Clone + Send + 'static,
>: Model
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
        source_event_id: EventMetadata,
        cx: &mut Context<Self>,
    ) -> impl Future<Output = ()> + Send;
}

pub trait ContStock<
    ResourceType: ContArithmetic + Clone + Send + 'static,
    StateType: StockState + Clone + Send + 'static,
> where
    Self: Model,
{
    fn get_state(&mut self) -> StateType;

    fn get_state_async(&mut self, _: (), _: &mut Context<Self>) -> impl Future<Output = StateType> + Send {
        async move {
            self.get_state()
        }
    }

    fn get_next_event_meta(&mut self) -> EventMetadata;

    fn previous_state(&mut self) -> &mut Option<StateType>;
    fn resource(&mut self) -> &mut ResourceType;
    fn state_emitter(&mut self) -> &mut Output<EventMetadata>;

    fn add(&mut self, payload: (ResourceType, EventMetadata), mut cx: &mut Context<Self>) -> impl Future<Output = ()> + Send {
        async move {
            *self.previous_state() = Some(self.get_state().clone());
            self.resource().add(payload.0.clone());
            let event_id = self.log_type_add(&mut payload.1.clone(), payload.0.clone(), &mut cx).await;

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

    fn remove<T: Send>(&mut self, payload: (T, EventMetadata), mut cx: &mut Context<Self>) -> impl Future<Output = ResourceType> + Send where ResourceType: Projectable<T> {
        async move {
            *self.previous_state() = Some(self.get_state().clone());
            let removed = self.resource().remove(payload.0);
            let event_id = self.log_type_remove(&mut payload.1.clone(), removed.clone(), &mut cx).await;

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

    fn remove_void<T: Send>(&mut self, payload: (T, EventMetadata), cx: &mut Context<Self>) -> impl Future<Output = ()> + Send where ResourceType: Projectable<T> {
        async move {
            self.remove(payload, cx).await;
        }
    }

    fn emit_change(&mut self, payload: (StateType, EventMetadata), cx: &mut Context<Self>) -> impl Future<Output = ()> + Send {
        async move {
            let event_id = self.log_type_state_change(&mut payload.1.clone(), payload.0.clone(), cx).await;
            self.state_emitter().send(event_id).await;
        }
    }

    fn log_type_add(&mut self, source_event_id: &mut EventMetadata, resource: ResourceType, cx: &mut Context<Self>) -> impl Future<Output = EventMetadata> + Send;
    fn log_type_remove(&mut self, source_event_id: &mut EventMetadata, resource: ResourceType, cx: &mut Context<Self>) -> impl Future<Output = EventMetadata> + Send;
    fn log_type_state_change(&mut self, source_event_id: &mut EventMetadata, new_state: StateType, cx: &mut Context<Self>) -> impl Future<Output = EventMetadata> + Send;
}