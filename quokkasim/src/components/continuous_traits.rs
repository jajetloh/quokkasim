use serde::Serialize;
use std::{fmt::Debug, time::Duration};

use crate::{
    common::{EventId, StockState},
    components::environment::BasicEnvironmentState,
    distributions::Distribution,
    nexosim::{ActionKey, Context, Model, MonotonicTime, Output, Requestor},
    prelude::ContStockState,
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
>: ContProcessCore<
    ResourceType,
    LogRecordType
>
where 
    ResourceType: ContResource + 'static,
    LogRecordType: Clone + Send + 'static,
{
    fn update_process_state_since_prev_event(
        &mut self, source_event_id: &mut EventId,
        cx: &mut Context<Self>,
        duration_since_prev: Duration
    ) -> impl Future<Output = ()>;

    fn update_state_since_last_update(
        &mut self,
        source_event_id: &mut EventId,
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
}

pub trait ContProcessUpdateDecisionLogic<
    ResourceType: ContResource + 'static,
    LogRecordType: Clone + Send + 'static,  
>: ContProcessCore<
    ResourceType,
    LogRecordType
> where
    ResourceType: ContResource + 'static,
    LogRecordType: Clone + Send + 'static,
{
    fn update_state_decision_logic(
        &mut self,
        source_event_id: &mut EventId,
        cx: &mut Context<Self>,
    ) -> impl Future<Output = ()>;
}

pub trait ContProcessUpdateForNextEvent<
    ResourceType: ContResource + 'static,
    LogRecordType: Clone + Send + 'static,
>
where
    Self: ContProcessCore<
        ResourceType,
        LogRecordType,
    >,
    ResourceType: ContResource + 'static,
    LogRecordType: Clone + Send + 'static,
{
    fn update_state_for_next_event(
        &mut self,
        source_event_id: &mut EventId,
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
    fn get_next_event_id(&mut self) -> EventId;
    fn scheduled_event(&mut self) -> &mut Option<(MonotonicTime, ActionKey)>;
    fn previous_check_time(&mut self) -> &mut MonotonicTime;
    fn process_state(&mut self) -> &mut Option<(Duration, ResourceType)>;
    fn time_to_next_process_event(&mut self) -> &mut Option<Duration>;

    fn log_type_withdraw_request(&mut self, source_event_id: &mut EventId, quantity: f64, cx: &mut Context<Self>) -> impl Future<Output = EventId>;
    fn log_type_process_start(&mut self, source_event_id: &mut EventId, quantity: f64, resource: ResourceType, cx: &mut Context<Self>) -> impl Future<Output = EventId>;
    fn log_type_process_success(&mut self, source_event_id: &mut EventId, quantity: f64, resource: ResourceType, cx: &mut Context<Self>) -> impl Future<Output = EventId>;
    fn log_type_process_failure(&mut self, source_event_id: &mut EventId, reason: &'static str, cx: &mut Context<Self>) -> impl Future<Output = EventId>;

    fn update_state(
        &mut self,
        source_event_id: EventId,
        cx: &mut Context<Self>,
    ) -> impl Future<Output = ()> + Send;
}

// pub trait ContProcessUpdateSinceLast<
//     ResourceType: ContResource + 'static,
//     LogRecordType: Clone + Send + 'static,
// >: ContProcessCore<
//     ResourceType,
//     LogRecordType
// >
// where
//     ResourceType: ContResource + 'static,
//     LogRecordType: Clone + Send + 'static,
// {
    
//     fn update_process_state_since_prev_event(
//         &mut self, source_event_id: &mut EventId,
//         cx: &mut Context<Self>,
//         duration_since_prev: Duration 
//     ) -> impl Future<Output = ()>;

//     fn update_state_since_last_update(
//         &mut self,
//         source_event_id: &mut EventId,
//         cx: &mut Context<Self>,
//     ) -> impl Future<Output = ()> {
//         async move {
//             if let Some((scheduled_time, _)) = self.scheduled_event() {
//                 if *scheduled_time <= cx.time() {
//                     *self.scheduled_event() = None;
//                 }
//             }

//             let duration_since_prev = cx.time().duration_since(*self.previous_check_time());

//             self.update_process_state_since_prev_event(source_event_id, cx, duration_since_prev).await;
//         }
//     }
// }

// pub trait ContProcessUpdateDecisionLogic<
//     ResourceType: ContResource + 'static,
//     LogRecordType: Clone + Send + 'static,
// >: ContProcessCore<
//     ResourceType,
//     LogRecordType
// > where
//     ResourceType: ContResource + 'static,
//     LogRecordType: Clone + Send + 'static,
// {
//     fn update_state_decision_logic(
//         &mut self,
//         source_event_id: &mut EventId,
//         cx: &mut Context<Self>,
//     ) -> impl Future<Output = ()> + Send;
// }

// pub trait ContProcess<
//     ResourceType: ContResource + 'static,
//     LogRecordType: Clone + Send + 'static,
// > where
//     Self: ContProcessCore<ResourceType, LogRecordType>
// {
//     fn req_upstream(&mut self) -> &mut Requestor<(), ContStockState>;
//     fn withdraw_upstream(&mut self) -> &mut Requestor<(f64, EventId), ResourceType>;
//     fn req_downstream(&mut self) -> &mut Requestor<(), ContStockState>;
//     fn push_downstream(&mut self) -> &mut Output<(ResourceType, EventId)>;
//     fn process_quantity_distr(&mut self) -> &mut Distribution;
//     fn process_time_distr(&mut self) -> &mut Distribution;

//     fn update_state(
//         &mut self,
//         mut source_event_id: EventId,
//         cx: &mut Context<Self>,
//     ) -> impl Future<Output = ()> {
//         async move {
//             self.update_state_since_last_update(&mut source_event_id, cx)
//                 .await;
//             self.update_state_decision_logic(&mut source_event_id, cx)
//                 .await;
//             self.update_state_next_event(&mut source_event_id, cx)
//                 .await;
//         }
//     }

//     fn update_process_state_since_prev_event(
//         &mut self, source_event_id: &mut EventId,
//         cx: &mut Context<Self>,
//         duration_since_prev: Duration
//     ) -> impl Future<Output = ()>;

//     // fn update_state_since_last_update(
//     //     &mut self,
//     //     source_event_id: &mut EventId,
//     //     cx: &mut Context<Self>,
//     // ) -> impl Future<Output = ()> {
//     //     async move {
//     //         if let Some((scheduled_time, _)) = self.scheduled_event() {
//     //             if *scheduled_time <= cx.time() {
//     //                 *self.scheduled_event() = None;
//     //             }
//     //         }

//     //         let duration_since_prev = cx.time().duration_since(*self.previous_check_time());

//     //         self.update_process_state_since_prev_event(source_event_id, cx, duration_since_prev).await;
//     //         // Update variables from elapsed time
//     //         // if let Some((scheduled_time, _)) = self.scheduled_event() {
//     //         //     if *scheduled_time <= cx.time() {
//     //         //         *self.scheduled_event() = None;
//     //         //     }
//     //         // }
//     //         // let time = cx.time();
//     //         // let duration_since_prev_check = cx.time().duration_since(*self.previous_check_time());
//     //         // {
//     //         //     let is_in_delay = self.delay_modes().active_delay().is_some();
//     //         //     let is_in_process = self.process_state().is_some() && !is_in_delay;
//     //         //     let is_env_blocked = matches!(self.env_state(), BasicEnvironmentState::Stopped);

//     //         //     // Decrement process time counter (if not delayed or env blocked)
//     //         //     if !(is_in_delay || is_env_blocked) {
//     //         //         if let Some((mut process_time_left, resource)) = self.process_state().take() {
//     //         //             process_time_left =
//     //         //                 process_time_left.saturating_sub(duration_since_prev_check);
//     //         //             if process_time_left.is_zero() {
//     //         //                 *source_event_id = self
//     //         //                     .log(
//     //         //                         time,
//     //         //                         source_event_id.clone(),
//     //         //                         self.log_type_process_success(
//     //         //                             resource.total(),
//     //         //                             resource.clone(),
//     //         //                         ),
//     //         //                     )
//     //         //                     .await;
//     //         //                 self.push_downstream()
//     //         //                     .send((resource.clone(), source_event_id.clone()))
//     //         //                     .await;
//     //         //             } else {
//     //         //                 *self.process_state() = Some((process_time_left, resource));
//     //         //             }
//     //         //         }
//     //         //     }

//     //         //     // Only case we don't update state here is if no delay is if we don't want the delay counters to decrement,
//     //         //     // which is only the case if we're not processing and not in a delay - i.e. time-until-delay counters only decrement
//     //         //     // when a process is active
//     //         //     if !is_env_blocked && (is_in_delay || is_in_process) {
//     //         //         let delay_transition =
//     //         //             self.delay_modes().update_state(duration_since_prev_check);
//     //         //         if delay_transition.has_changed() {
//     //         //             if let Some(delay_name) = &delay_transition.from {
//     //         //                 *source_event_id = self
//     //         //                     .log(
//     //         //                         time,
//     //         //                         source_event_id.clone(),
//     //         //                         self.log_type_delay_end(delay_name.clone()),
//     //         //                     )
//     //         //                     .await;
//     //         //             }
//     //         //             if let Some(delay_name) = &delay_transition.to {
//     //         //                 *source_event_id = self
//     //         //                     .log(
//     //         //                         time,
//     //         //                         source_event_id.clone(),
//     //         //                         self.log_type_delay_start(delay_name.clone()),
//     //         //                     )
//     //         //                     .await;
//     //         //             }
//     //         //         }
//     //         //     }
//     //         // }
//     //     }
//     // }

//     fn update_state_decision_logic(
//         &mut self,
//         source_event_id: &mut EventId,
//         cx: &mut Context<Self>,
//     ) -> impl Future<Output = ()> + Send;
//     //  {
//     //     async move {
//     //         let time = cx.time();
//     //         {
//     //             let new_env_state = match self.req_environment().send(()).await.next() {
//     //                 Some(x) => x,
//     //                 None => BasicEnvironmentState::Normal, // Assume always normal operation if no environment state connected
//     //             };
//     //             match (&self.env_state(), &new_env_state) {
//     //                 (BasicEnvironmentState::Normal, BasicEnvironmentState::Stopped) => {
//     //                     *source_event_id = self
//     //                         .log(
//     //                             time,
//     //                             source_event_id.clone(),
//     //                             self.log_type_process_stopped("Stopped by environment"),
//     //                         )
//     //                         .await;
//     //                     *self.env_state() = BasicEnvironmentState::Stopped;
//     //                 }
//     //                 (BasicEnvironmentState::Stopped, BasicEnvironmentState::Normal) => {
//     //                     *source_event_id = self
//     //                         .log(
//     //                             time,
//     //                             source_event_id.clone(),
//     //                             self.log_type_process_continue("Resumed by environment"),
//     //                         )
//     //                         .await;
//     //                     *self.env_state() = BasicEnvironmentState::Normal;
//     //                 }
//     //                 _ => {}
//     //             }
//     //         }

//     //         // Update internal state
//     //         let is_env_stopped = matches!(self.env_state(), BasicEnvironmentState::Stopped);
//     //         let has_active_delay = self.delay_modes().active_delay().is_some() || is_env_stopped;
//     //         match (&self.process_state(), has_active_delay) {
//     //             (None, false) => {
//     //                 let us_state = self.req_upstream().send(()).await.next();
//     //                 let ds_state = self.req_downstream().send(()).await.next();

//     //                 match (&us_state, &ds_state) {
//     //                     (
//     //                         Some(ContStockState::Normal { .. })
//     //                         | Some(ContStockState::Full { .. }),
//     //                         Some(ContStockState::Empty { .. })
//     //                         | Some(ContStockState::Normal { .. }),
//     //                     ) => {
//     //                         let process_quantity = self.process_quantity_distr().sample();
//     //                         *source_event_id = self
//     //                             .log(
//     //                                 time,
//     //                                 source_event_id.clone(),
//     //                                 self.log_type_withdraw_request(),
//     //                             )
//     //                             .await;
//     //                         let moved = self
//     //                             .withdraw_upstream()
//     //                             .send((process_quantity, source_event_id.clone()))
//     //                             .await
//     //                             .next()
//     //                             .unwrap();
//     //                         let process_duration_secs = self.process_time_distr().sample();
//     //                         *self.process_state() = Some((
//     //                             Duration::from_secs_f64(process_duration_secs),
//     //                             moved.clone(),
//     //                         ));
//     //                         *source_event_id = self
//     //                             .log(
//     //                                 time,
//     //                                 source_event_id.clone(),
//     //                                 self.log_type_process_start(process_quantity, moved),
//     //                             )
//     //                             .await;
//     //                         *self.time_to_next_process_event() =
//     //                             Some(Duration::from_secs_f64(process_duration_secs));
//     //                     }
//     //                     (Some(ContStockState::Empty { .. }), _) => {
//     //                         *source_event_id = self
//     //                             .log(
//     //                                 time,
//     //                                 source_event_id.clone(),
//     //                                 self.log_type_process_failure("Upstream is empty"),
//     //                             )
//     //                             .await;
//     //                         *self.time_to_next_process_event() = None;
//     //                     }
//     //                     (None, _) => {
//     //                         *source_event_id = self
//     //                             .log(
//     //                                 time,
//     //                                 source_event_id.clone(),
//     //                                 self.log_type_process_failure("Upstream is not connected"),
//     //                             )
//     //                             .await;
//     //                         *self.time_to_next_process_event() = None;
//     //                     }
//     //                     (_, None) => {
//     //                         *source_event_id = self
//     //                             .log(
//     //                                 time,
//     //                                 source_event_id.clone(),
//     //                                 self.log_type_process_failure("Downstream is not connected"),
//     //                             )
//     //                             .await;
//     //                         *self.time_to_next_process_event() = None;
//     //                     }
//     //                     (_, Some(ContStockState::Full { .. })) => {
//     //                         *source_event_id = self
//     //                             .log(
//     //                                 time,
//     //                                 source_event_id.clone(),
//     //                                 self.log_type_process_failure("Downstream is full"),
//     //                             )
//     //                             .await;
//     //                         *self.time_to_next_process_event() = None;
//     //                     }
//     //                 }
//     //             }
//     //             (Some((time, _)), false) => {
//     //                 *self.time_to_next_process_event() = Some(*time);
//     //             }
//     //             (_, true) => {
//     //                 *self.time_to_next_process_event() = self
//     //                     .delay_modes()
//     //                     .active_delay()
//     //                     .map(|(_, delay_state)| *delay_state);
//     //             }
//     //         }
//     //     }
//     // }

//     fn update_state_next_event(
//         &mut self,
//         source_event_id: &mut EventId,
//         cx: &mut Context<Self>,
//     ) -> impl Future<Output = ()> + Send {
//         async move {
//             let next_event = self.time_to_next_process_event();
//             if let Some(time_until_next) = next_event {
//                 if time_until_next.is_zero() {
//                     panic!("Time until next event is zero!");
//                 } else {
//                     let next_time = cx.time() + *time_until_next;

//                     // Schedule event if sooner. If so, cancel previous event.
//                     if let Some((scheduled_time, action_key)) = self.scheduled_event().take() {
//                         if next_time < scheduled_time {
//                             action_key.cancel();
//                             let new_event_key = cx
//                                 .schedule_keyed_event(
//                                     next_time,
//                                     Self::update_state,
//                                     source_event_id.clone(),
//                                 )
//                                 .unwrap();
//                             *self.scheduled_event() = Some((next_time, new_event_key));
//                         } else {
//                             // Put the event back
//                             *self.scheduled_event() = Some((scheduled_time, action_key));
//                         }
//                     } else {
//                         let new_event_key = cx
//                             .schedule_keyed_event(
//                                 next_time,
//                                 Self::update_state,
//                                 source_event_id.clone(),
//                             )
//                             .unwrap();
//                         *self.scheduled_event() = Some((next_time, new_event_key));
//                     }
//                 };
//             }
//             *self.previous_check_time() = cx.time();
//         }
//     }
// }

// pub trait Source<
//     ResourceType: ContResource + 'static,
//     LogRecordType: Clone + Send + 'static,
// > where
//     Self: ContProcessCore<ResourceType, LogRecordType>
// {
//     fn req_downstream(&mut self) -> &mut Requestor<(), ContStockState>;
//     fn push_downstream(&mut self) -> &mut Output<(ResourceType, EventId)>;
//     fn source_resource(&mut self) -> &mut ResourceType;
//     fn source_quantity_distr(&mut self) -> &mut Distribution;
//     fn source_time_distr(&mut self) -> &mut Distribution;

//     fn update_state(
//         &mut self,
//         mut source_event_id: EventId,
//         cx: &mut Context<Self>,
//     ) -> impl Future<Output = ()> + Send {
//         async move {
//             self.update_state_since_last_update(&mut source_event_id, cx)
//                 .await;
//             self.update_state_decision_logic(&mut source_event_id, cx)
//                 .await;
//             self.update_state_next_event(&mut source_event_id, cx)
//                 .await;
//         }
//     }

//     fn update_state_since_last_update(
//         &mut self,
//         source_event_id: &mut EventId,
//         cx: &mut Context<Self>,
//     ) -> impl Future<Output = ()> + Send {
//         async move {
//             // Update variables from elapsed time
//             if let Some((scheduled_time, _)) = self.scheduled_event() {
//                 if *scheduled_time <= cx.time() {
//                     *self.scheduled_event() = None;
//                 }
//             }
//             let time = cx.time();
//             let duration_since_prev_check = cx.time().duration_since(*self.previous_check_time());
//             {
//                 let is_in_delay = self.delay_modes().active_delay().is_some();
//                 let is_in_process = self.process_state().is_some() && !is_in_delay;
//                 let is_env_blocked = matches!(self.env_state(), BasicEnvironmentState::Stopped);

//                 // Decrement process time counter (if not delayed or env blocked)
//                 if !(is_in_delay || is_env_blocked) {
//                     if let Some((mut process_time_left, resource)) = self.process_state().take() {
//                         process_time_left =
//                             process_time_left.saturating_sub(duration_since_prev_check);
//                         if process_time_left.is_zero() {
//                             *source_event_id = self
//                                 .log(
//                                     time,
//                                     source_event_id.clone(),
//                                     self.log_type_process_success(
//                                         resource.total(),
//                                         resource.clone(),
//                                     ),
//                                 )
//                                 .await;
//                             self.push_downstream()
//                                 .send((resource.clone(), source_event_id.clone()))
//                                 .await;
//                         } else {
//                             *self.process_state() = Some((process_time_left, resource));
//                         }
//                     }
//                 }

//                 // Only case we don't update state here is if no delay is if we don't want the delay counters to decrement,
//                 // which is only the case if we're not processing and not in a delay - i.e. time-until-delay counters only decrement
//                 // when a process is active
//                 if !is_env_blocked && (is_in_delay || is_in_process) {
//                     let delay_transition =
//                         self.delay_modes().update_state(duration_since_prev_check);
//                     if delay_transition.has_changed() {
//                         if let Some(delay_name) = &delay_transition.from {
//                             *source_event_id = self
//                                 .log(
//                                     time,
//                                     source_event_id.clone(),
//                                     self.log_type_delay_end(delay_name.clone()),
//                                 )
//                                 .await;
//                         }
//                         if let Some(delay_name) = &delay_transition.to {
//                             *source_event_id = self
//                                 .log(
//                                     time,
//                                     source_event_id.clone(),
//                                     self.log_type_delay_start(delay_name.clone()),
//                                 )
//                                 .await;
//                         }
//                     }
//                 }
//             }
//         }
//     }

//     fn update_state_decision_logic(
//         &mut self,
//         source_event_id: &mut EventId,
//         cx: &mut Context<Self>,
//     ) -> impl Future<Output = ()> + Send {
//         async move {
//             let time = cx.time();
//             {
//                 let new_env_state = match self.req_environment().send(()).await.next() {
//                     Some(x) => x,
//                     None => BasicEnvironmentState::Normal, // Assume always normal operation if no environment state connected
//                 };
//                 match (&self.env_state(), &new_env_state) {
//                     (BasicEnvironmentState::Normal, BasicEnvironmentState::Stopped) => {
//                         *source_event_id = self
//                             .log(
//                                 time,
//                                 source_event_id.clone(),
//                                 self.log_type_process_stopped("Stopped by environment"),
//                             )
//                             .await;
//                         *self.env_state() = BasicEnvironmentState::Stopped;
//                     }
//                     (BasicEnvironmentState::Stopped, BasicEnvironmentState::Normal) => {
//                         *source_event_id = self
//                             .log(
//                                 time,
//                                 source_event_id.clone(),
//                                 self.log_type_process_continue("Resumed by environment"),
//                             )
//                             .await;
//                         *self.env_state() = BasicEnvironmentState::Normal;
//                     }
//                     _ => {}
//                 }
//             }

//             // Update internal state
//             let is_env_stopped = matches!(self.env_state(), BasicEnvironmentState::Stopped);
//             let has_active_delay = self.delay_modes().active_delay().is_some() || is_env_stopped;
//             match (&self.process_state(), has_active_delay) {
//                 (None, false) => {
//                     let ds_state = self.req_downstream().send(()).await.next();

//                     match &ds_state {
//                         Some(ContStockState::Empty { .. })
//                         | Some(ContStockState::Normal { .. }) => {
//                             let process_quantity = self.source_quantity_distr().sample();
//                             let mut created_resource = self.source_resource().clone();
//                             created_resource.multiply(process_quantity / self.source_resource().total());

//                             *source_event_id = self
//                                 .log(
//                                     time,
//                                     source_event_id.clone(),
//                                     self.log_type_withdraw_request(),
//                                 )
//                                 .await;
//                             let process_duration_secs = self.source_time_distr().sample();
//                             *self.process_state() = Some((
//                                 Duration::from_secs_f64(process_duration_secs),
//                                 created_resource.clone()
//                             ));
//                             *source_event_id = self
//                                 .log(
//                                     time,
//                                     source_event_id.clone(),
//                                     self.log_type_process_start(process_quantity, created_resource),
//                                 )
//                                 .await;
//                             *self.time_to_next_process_event() =
//                                 Some(Duration::from_secs_f64(process_duration_secs));
//                         }
//                         None => {
//                             *source_event_id = self
//                                 .log(
//                                     time,
//                                     source_event_id.clone(),
//                                     self.log_type_process_failure("Downstream is not connected"),
//                                 )
//                                 .await;
//                             *self.time_to_next_process_event() = None;
//                         }
//                         Some(ContStockState::Full { .. }) => {
//                             *source_event_id = self
//                                 .log(
//                                     time,
//                                     source_event_id.clone(),
//                                     self.log_type_process_failure("Downstream is full"),
//                                 )
//                                 .await;
//                             *self.time_to_next_process_event() = None;
//                         }
//                     }
//                 }
//                 (Some((time, _)), false) => {
//                     *self.time_to_next_process_event() = Some(*time);
//                 }
//                 (_, true) => {
//                     *self.time_to_next_process_event() = self
//                         .delay_modes()
//                         .active_delay()
//                         .map(|(_, delay_state)| *delay_state);
//                 }
//             }
//         }
//     }

//     fn update_state_next_event(
//         &mut self,
//         source_event_id: &mut EventId,
//         cx: &mut Context<Self>,
//     ) -> impl Future<Output = ()> + Send {
//         async move {
//             let is_env_stopped = matches!(self.env_state(), BasicEnvironmentState::Stopped);
//             let has_active_delay = self.delay_modes().active_delay().is_some() || is_env_stopped;

//             if self.process_state().is_some() || has_active_delay || !is_env_stopped {
//                 *self.time_to_next_delay_event() = self
//                     .delay_modes()
//                     .get_next_event()
//                     .map(|(_, delay_state)| delay_state.as_duration());
//             } else {
//                 *self.time_to_next_delay_event() = None;
//             }
//             let time_to_next_event = [
//                 *self.time_to_next_delay_event(),
//                 *self.time_to_next_process_event(),
//             ]
//             .into_iter()
//             .flatten()
//             .min();
//             match time_to_next_event {
//                 None => {}
//                 Some(time_until_next) => {
//                     if time_until_next.is_zero() {
//                         panic!("Time until next event is zero!");
//                     } else {
//                         let next_time = cx.time() + time_until_next;

//                         // Schedule event if sooner. If so, cancel previous event.
//                         if let Some((scheduled_time, action_key)) = self.scheduled_event().take() {
//                             if next_time < scheduled_time {
//                                 action_key.cancel();
//                                 let new_event_key = cx
//                                     .schedule_keyed_event(
//                                         next_time,
//                                         Self::update_state,
//                                         source_event_id.clone(),
//                                     )
//                                     .unwrap();
//                                 *self.scheduled_event() = Some((next_time, new_event_key));
//                             } else {
//                                 // Put the event back
//                                 *self.scheduled_event() = Some((scheduled_time, action_key));
//                             }
//                         } else {
//                             let new_event_key = cx
//                                 .schedule_keyed_event(
//                                     next_time,
//                                     Self::update_state,
//                                     source_event_id.clone(),
//                                 )
//                                 .unwrap();
//                             *self.scheduled_event() = Some((next_time, new_event_key));
//                         }
//                     };
//                 }
//             };
//             *self.previous_check_time() = cx.time();
//         }
//     }
// }

// pub trait Sink<
//     ResourceType: ContResource + 'static,
//     LogRecordType: Clone + Send + 'static,
//     LogDetailsType: Clone + Send + 'static,
// > where
//     Self: ContProcessCore<ResourceType, LogRecordType, LogDetailsType>
// {
//     fn req_upstream(&mut self) -> &mut Requestor<(), ContStockState>;
//     fn withdraw_upstream(&mut self) -> &mut Requestor<(f64, EventId), ResourceType>;
//     fn sink_quantity_distr(&mut self) -> &mut Distribution;
//     fn sink_time_distr(&mut self) -> &mut Distribution;

//     fn update_state(
//         &mut self,
//         mut source_event_id: EventId,
//         cx: &mut Context<Self>,
//     ) -> impl Future<Output = ()> + Send {
//         async move {
//             self.update_state_since_last_update(&mut source_event_id, cx)
//                 .await;
//             self.update_state_decision_logic(&mut source_event_id, cx)
//                 .await;
//             self.update_state_next_event(&mut source_event_id, cx)
//                 .await;
//         }
//     }

//     fn update_state_since_last_update(
//         &mut self,
//         source_event_id: &mut EventId,
//         cx: &mut Context<Self>,
//     ) -> impl Future<Output = ()> + Send {
//         async move {
//             // Update variables from elapsed time
//             if let Some((scheduled_time, _)) = self.scheduled_event() {
//                 if *scheduled_time <= cx.time() {
//                     *self.scheduled_event() = None;
//                 }
//             }
//             let time = cx.time();
//             let duration_since_prev_check = cx.time().duration_since(*self.previous_check_time());
//             {
//                 let is_in_delay = self.delay_modes().active_delay().is_some();
//                 let is_in_process = self.process_state().is_some() && !is_in_delay;
//                 let is_env_blocked = matches!(self.env_state(), BasicEnvironmentState::Stopped);

//                 // Decrement process time counter (if not delayed or env blocked)
//                 if !(is_in_delay || is_env_blocked) {
//                     if let Some((mut process_time_left, resource)) = self.process_state().take() {
//                         process_time_left =
//                             process_time_left.saturating_sub(duration_since_prev_check);
//                         if process_time_left.is_zero() {
//                             *source_event_id = self
//                                 .log(
//                                     time,
//                                     source_event_id.clone(),
//                                     self.log_type_process_success(
//                                         resource.total(),
//                                         resource.clone(),
//                                     ),
//                                 )
//                                 .await;
//                         } else {
//                             *self.process_state() = Some((process_time_left, resource));
//                         }
//                     }
//                 }

//                 // Only case we don't update state here is if no delay is if we don't want the delay counters to decrement,
//                 // which is only the case if we're not processing and not in a delay - i.e. time-until-delay counters only decrement
//                 // when a process is active
//                 if !is_env_blocked && (is_in_delay || is_in_process) {
//                     let delay_transition =
//                         self.delay_modes().update_state(duration_since_prev_check);
//                     if delay_transition.has_changed() {
//                         if let Some(delay_name) = &delay_transition.from {
//                             *source_event_id = self
//                                 .log(
//                                     time,
//                                     source_event_id.clone(),
//                                     self.log_type_delay_end(delay_name.clone()),
//                                 )
//                                 .await;
//                         }
//                         if let Some(delay_name) = &delay_transition.to {
//                             *source_event_id = self
//                                 .log(
//                                     time,
//                                     source_event_id.clone(),
//                                     self.log_type_delay_start(delay_name.clone()),
//                                 )
//                                 .await;
//                         }
//                     }
//                 }
//             }
//         }
//     }

//     fn update_state_decision_logic(
//         &mut self,
//         source_event_id: &mut EventId,
//         cx: &mut Context<Self>,
//     ) -> impl Future<Output = ()> + Send {
//         async move {
//             let time = cx.time();
//             {
//                 let new_env_state = match self.req_environment().send(()).await.next() {
//                     Some(x) => x,
//                     None => BasicEnvironmentState::Normal, // Assume always normal operation if no environment state connected
//                 };
//                 match (&self.env_state(), &new_env_state) {
//                     (BasicEnvironmentState::Normal, BasicEnvironmentState::Stopped) => {
//                         *source_event_id = self
//                             .log(
//                                 time,
//                                 source_event_id.clone(),
//                                 self.log_type_process_stopped("Stopped by environment"),
//                             )
//                             .await;
//                         *self.env_state() = BasicEnvironmentState::Stopped;
//                     }
//                     (BasicEnvironmentState::Stopped, BasicEnvironmentState::Normal) => {
//                         *source_event_id = self
//                             .log(
//                                 time,
//                                 source_event_id.clone(),
//                                 self.log_type_process_continue("Resumed by environment"),
//                             )
//                             .await;
//                         *self.env_state() = BasicEnvironmentState::Normal;
//                     }
//                     _ => {}
//                 }
//             }

//             // Update internal state
//             let is_env_stopped = matches!(self.env_state(), BasicEnvironmentState::Stopped);
//             let has_active_delay = self.delay_modes().active_delay().is_some() || is_env_stopped;
//             match (&self.process_state(), has_active_delay) {
//                 (None, false) => {
//                     let us_state = self.req_upstream().send(()).await.next();

//                     match &us_state {
//                         Some(ContStockState::Empty { .. })
//                         | Some(ContStockState::Normal { .. }) => {
//                             let process_quantity = self.sink_quantity_distr().sample();
//                             let moved = self.withdraw_upstream()
//                                 .send((process_quantity, source_event_id.clone()))
//                                 .await
//                                 .next()
//                                 .unwrap();

//                             *source_event_id = self
//                                 .log(
//                                     time,
//                                     source_event_id.clone(),
//                                     self.log_type_withdraw_request(),
//                                 )
//                                 .await;
//                             let process_duration_secs = self.sink_time_distr().sample();
//                             *self.process_state() = Some((
//                                 Duration::from_secs_f64(process_duration_secs),
//                                 moved.clone()
//                             ));
//                             *source_event_id = self
//                                 .log(
//                                     time,
//                                     source_event_id.clone(),
//                                     self.log_type_process_start(process_quantity, moved),
//                                 )
//                                 .await;
//                             *self.time_to_next_process_event() =
//                                 Some(Duration::from_secs_f64(process_duration_secs));
//                         }
//                         None => {
//                             *source_event_id = self
//                                 .log(
//                                     time,
//                                     source_event_id.clone(),
//                                     self.log_type_process_failure("Downstream is not connected"),
//                                 )
//                                 .await;
//                             *self.time_to_next_process_event() = None;
//                         }
//                         Some(ContStockState::Full { .. }) => {
//                             *source_event_id = self
//                                 .log(
//                                     time,
//                                     source_event_id.clone(),
//                                     self.log_type_process_failure("Downstream is full"),
//                                 )
//                                 .await;
//                             *self.time_to_next_process_event() = None;
//                         }
//                     }
//                 }
//                 (Some((time, _)), false) => {
//                     *self.time_to_next_process_event() = Some(*time);
//                 }
//                 (_, true) => {
//                     *self.time_to_next_process_event() = self
//                         .delay_modes()
//                         .active_delay()
//                         .map(|(_, delay_state)| *delay_state);
//                 }
//             }
//         }
//     }

//     fn update_state_next_event(
//         &mut self,
//         source_event_id: &mut EventId,
//         cx: &mut Context<Self>,
//     ) -> impl Future<Output = ()> + Send {
//         async move {
//             let is_env_stopped = matches!(self.env_state(), BasicEnvironmentState::Stopped);
//             let has_active_delay = self.delay_modes().active_delay().is_some() || is_env_stopped;

//             if self.process_state().is_some() || has_active_delay || !is_env_stopped {
//                 *self.time_to_next_delay_event() = self
//                     .delay_modes()
//                     .get_next_event()
//                     .map(|(_, delay_state)| delay_state.as_duration());
//             } else {
//                 *self.time_to_next_delay_event() = None;
//             }
//             let time_to_next_event = [
//                 *self.time_to_next_delay_event(),
//                 *self.time_to_next_process_event(),
//             ]
//             .into_iter()
//             .flatten()
//             .min();
//             match time_to_next_event {
//                 None => {}
//                 Some(time_until_next) => {
//                     if time_until_next.is_zero() {
//                         panic!("Time until next event is zero!");
//                     } else {
//                         let next_time = cx.time() + time_until_next;

//                         // Schedule event if sooner. If so, cancel previous event.
//                         if let Some((scheduled_time, action_key)) = self.scheduled_event().take() {
//                             if next_time < scheduled_time {
//                                 action_key.cancel();
//                                 let new_event_key = cx
//                                     .schedule_keyed_event(
//                                         next_time,
//                                         Self::update_state,
//                                         source_event_id.clone(),
//                                     )
//                                     .unwrap();
//                                 *self.scheduled_event() = Some((next_time, new_event_key));
//                             } else {
//                                 // Put the event back
//                                 *self.scheduled_event() = Some((scheduled_time, action_key));
//                             }
//                         } else {
//                             let new_event_key = cx
//                                 .schedule_keyed_event(
//                                     next_time,
//                                     Self::update_state,
//                                     source_event_id.clone(),
//                                 )
//                                 .unwrap();
//                             *self.scheduled_event() = Some((next_time, new_event_key));
//                         }
//                     };
//                 }
//             };
//             *self.previous_check_time() = cx.time();
//         }
//     }
// }


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

    fn get_next_event_id(&mut self) -> EventId;

    fn previous_state(&mut self) -> &mut Option<StateType>;
    fn resource(&mut self) -> &mut ResourceType;
    fn state_emitter(&mut self) -> &mut Output<EventId>;

    fn add(&mut self, payload: (ResourceType, EventId), mut cx: &mut Context<Self>) -> impl Future<Output = ()> + Send {
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

    fn remove<T: Send>(&mut self, payload: (T, EventId), mut cx: &mut Context<Self>) -> impl Future<Output = ResourceType> + Send where ResourceType: Projectable<T> {
        async move {
            *self.previous_state() = Some(self.get_state().clone());
            let removed = self.resource().remove(payload.0);
            // let event_id = self.log(
            //         cx.time(),
            //         payload.1.clone(),
            //         self.log_type_remove(total, removed.clone())
            //     )
            //     .await;
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

    fn remove_void<T: Send>(&mut self, payload: (T, EventId), cx: &mut Context<Self>) -> impl Future<Output = ()> + Send where ResourceType: Projectable<T> {
        async move {
            self.remove(payload, cx).await;
        }
    }

    fn emit_change(&mut self, payload: (StateType, EventId), cx: &mut Context<Self>) -> impl Future<Output = ()> + Send {
        async move {
            let event_id = self.log_type_state_change(&mut payload.1.clone(), payload.0.clone(), cx).await;
            self.state_emitter().send(event_id).await;
        }
    }

    fn log_type_add(&mut self, source_event_id: &mut EventId, resource: ResourceType, cx: &mut Context<Self>) -> impl Future<Output = EventId> + Send;
    fn log_type_remove(&mut self, source_event_id: &mut EventId, resource: ResourceType, cx: &mut Context<Self>) -> impl Future<Output = EventId> + Send;
    fn log_type_state_change(&mut self, source_event_id: &mut EventId, new_state: StateType, cx: &mut Context<Self>) -> impl Future<Output = EventId> + Send;
}