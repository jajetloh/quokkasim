use std::time::Duration;

use quokkasim::prelude::*;

use crate::iron_ore::IronOre;
use crate::loggers::{TruckingProcessLog, TruckingProcessLogType};
use crate::truck::Truck;

pub struct LoadingProcess {
    pub element_name: String,
    pub element_code: String,
    pub element_type: String,
    pub req_upstream_trucks: Requestor<(), DiscreteStockState>,
    pub req_upstream_ore: Requestor<(), VectorStockState>,
    pub req_downstream_trucks: Requestor<(), DiscreteStockState>,

    pub withdraw_upstream_trucks: Requestor<((), NotificationMetadata), Option<Truck>>,
    pub withdraw_upstream_ore: Requestor<(f64, NotificationMetadata), IronOre>,
    pub push_downstream_trucks: Output<(Truck, NotificationMetadata)>,

    pub process_state: Option<(Duration, Truck, IronOre)>,
    pub process_quantity_distr: Distribution,
    pub process_time_distr: Distribution,
    time_to_next_event: Option<Duration>,
    scheduled_event: Option<(MonotonicTime, ActionKey)>,
    next_event_index: u64,
    pub log_emitter: Output<TruckingProcessLog>,
    pub previous_check_time: MonotonicTime,
}

// Utility methods
impl LoadingProcess {
    pub fn new() -> Self {
        LoadingProcess {
            element_name: String::new(),
            element_code: String::new(),
            element_type: String::new(),
            req_upstream_trucks: Requestor::default(),
            req_upstream_ore: Requestor::default(),
            req_downstream_trucks: Requestor::default(),
            withdraw_upstream_trucks: Requestor::default(),
            withdraw_upstream_ore: Requestor::default(),
            push_downstream_trucks: Output::default(),
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

    pub fn with_name(mut self, name: String) -> Self {
        self.element_name = name;
        self
    }

    pub fn with_type(mut self, element_type: String) -> Self {
        self.element_type = element_type;
        self
    }

    pub fn with_process_quantity_distr(mut self, distr: Distribution) -> Self {
        self.process_quantity_distr = distr;
        self
    }

    pub fn with_process_time_distr(mut self, distr: Distribution) -> Self {
        self.process_time_distr = distr;
        self
    }
}

impl Model for LoadingProcess {}

impl LoadingProcess {
    pub fn update_state<'a> (&'a mut self, mut notif_meta: NotificationMetadata, cx: &'a mut Context<Self>) -> impl Future<Output = ()> + Send + 'a {
        async move {
            let time = cx.time();

            // Resolve current loading action if pending. Take the state - if time still remains before the event is due, we'll put it back
            match self.process_state.take() {
                Some((mut process_time_left, mut truck, ore_to_load)) => {
                    let duration_since_prev_check = time.duration_since(self.previous_check_time);
                    process_time_left = process_time_left.saturating_sub(duration_since_prev_check);
                    if process_time_left.is_zero() {
                        notif_meta = self.log(time, notif_meta.source_event, "Truck loaded", TruckingProcessLogType::LoadingSuccess { truck_id: truck.truck_id.clone(), quantity: ore_to_load.total(), ore: ore_to_load.clone() }).await;
                        match &mut truck.ore {
                            Some(existing_ore) => {
                                existing_ore.add(ore_to_load);
                            },
                            None => {
                                truck.ore = Some(ore_to_load);
                            }
                        }
                        self.push_downstream_trucks.send((truck, notif_meta.clone())).await;
                    } else {
                        self.process_state = Some((process_time_left, truck, ore_to_load));
                    }
                }
                None => {
                }
            }

            // Check if we need to start a new loading process
            match self.process_state {
                Some((time_left, _, _)) => {
                    self.time_to_next_event = Some(time_left);
                }
                None => {
                    // Check if we have trucks and ore available
                    let us_trucks_state = self.req_upstream_trucks.send(()).await.next();
                    let us_ore_state = self.req_upstream_ore.send(()).await.next();
                    let ds_trucks_state = self.req_downstream_trucks.send(()).await.next();

                    match (&us_trucks_state, &us_ore_state, &ds_trucks_state) {
                        (
                            Some(DiscreteStockState::Normal { .. }) | Some(DiscreteStockState::Full { .. }),
                            Some(VectorStockState::Normal { .. }) | Some(VectorStockState::Full { .. }),
                            Some(DiscreteStockState::Empty { .. }) | Some(DiscreteStockState::Normal { .. }),
                        ) => {

                            notif_meta = self.log(time, notif_meta.source_event, "Withdrawing for loading", TruckingProcessLogType::WithdrawRequest).await;
                            let mut truck = self.withdraw_upstream_trucks.send(((), notif_meta.clone())).await.next().unwrap();

                            match truck.take() {
                                Some(Truck { ore: None, truck_id }) => {
                                let process_quantity = self.process_quantity_distr.sample();
                                    let ore = self.withdraw_upstream_ore.send((process_quantity, notif_meta.clone())).await.next().unwrap();

                                    notif_meta = self.log(time, notif_meta.source_event, "Loading started", TruckingProcessLogType::LoadingStart { truck_id: truck_id.clone(), quantity: ore.total(), ore: ore.clone() }).await;

                                    let process_duration = self.process_time_distr.sample();
                                    self.process_state = Some((Duration::from_secs_f64(process_duration), Truck { ore: None, truck_id: truck_id.clone() }, ore.clone()));

                                    self.time_to_next_event = Some(Duration::from_secs_f64(process_duration));
                                },
                                Some(Truck { ore: Some(_), .. }) => {
                                    // Truck already has ore - how did we get here?
                                    panic!("Truck already has ore loaded, cannot load again");
                                }
                                None => {
                                    // No truck available upstream
                                    notif_meta = self.log(time, notif_meta.source_event, "No truck available upstream", TruckingProcessLogType::LoadingFailure { reason: "No truck available upstream" }).await;
                                    self.time_to_next_event = None;
                                }
                            }
                        }
                        (_, Some(VectorStockState::Empty { .. }) | None, _) => {
                            // No ore available upstream
                            notif_meta = self.log(time, notif_meta.source_event, "No ore available upstream", TruckingProcessLogType::LoadingFailure { reason: "No ore available upstream" }).await;
                            self.time_to_next_event = None;
                        },
                        (Some(DiscreteStockState::Empty { .. }) | None, _, _) => {
                            // Downstream truck stock is full
                            notif_meta = self.log(time, notif_meta.source_event, "Upstream truck stock is empty or not connected", TruckingProcessLogType::LoadingFailure { reason: "Upstream truck stock is empty or not connected" }).await;
                            self.time_to_next_event = None;
                        },
                        (_, _, Some(DiscreteStockState::Full { .. }) | None) => {
                            // Downstream truck stock is full
                            notif_meta = self.log(time, notif_meta.source_event, "Downstream truck stock is full or not connected", TruckingProcessLogType::LoadingFailure { reason: "Downstream truck stock is full or not connected" }).await;
                            self.time_to_next_event = None;
                        },
                    }
                }
            }
            
            self.previous_check_time = time;
            match self.time_to_next_event {
                Some(time_until_next) if time_until_next > Duration::ZERO => {
                    let next_time = time + time_until_next;

                    // Schedule event if sooner. If so, cancel previous event.
                    if let Some((scheduled_time, action_key)) = self.scheduled_event.take() {
                        if next_time < scheduled_time {
                            action_key.cancel();
                            let new_event_key =  cx.schedule_keyed_event(next_time, Self::update_state, notif_meta.clone()).unwrap();
                            self.scheduled_event = Some((next_time, new_event_key));
                        } else {
                            // Put the event back
                            self.scheduled_event = Some((scheduled_time, action_key));
                        }
                    } else {
                        let new_event_key =  cx.schedule_keyed_event(next_time, Self::update_state, notif_meta.clone()).unwrap();
                        self.scheduled_event = Some((next_time, new_event_key));
                    }
                }
                _ => {
                    // No further events scheduled
                }
            }
        }

    }

    pub fn log(&mut self, time: MonotonicTime, source_event: String, message: &'static str, log_type: TruckingProcessLogType) -> impl Future<Output = NotificationMetadata> + Send {
        async move {
            let event_id = format!("{}_{:06}", self.element_code, self.next_event_index);
            let log = TruckingProcessLog {
                time: time.to_chrono_date_time(0).unwrap().to_string(),
                event_id: event_id.clone(),
                source_event_id: source_event,
                element_name: self.element_name.clone(),
                element_type: self.element_type.clone(),
                event: log_type,
            };
            self.log_emitter.send(log).await;
            self.next_event_index += 1;

            NotificationMetadata {
                time,
                source_event: event_id,
                message,
            }
        }
    }
}
