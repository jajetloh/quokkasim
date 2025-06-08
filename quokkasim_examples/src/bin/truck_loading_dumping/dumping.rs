use std::time::Duration;

use quokkasim::prelude::*;

use crate::iron_ore::IronOre;
use crate::loggers::{TruckingProcessLog, TruckingProcessLogType};
use crate::truck::Truck;

pub struct DumpingProcess {
    pub element_name: String,
    pub element_code: String,
    pub element_type: String,
    pub req_upstream_trucks: Requestor<(), DiscreteStockState>,
    pub req_downstream_trucks: Requestor<(), DiscreteStockState>,
    pub req_downstream_ore: Requestor<(), VectorStockState>,

    pub withdraw_upstream_trucks: Requestor<((), EventId), Option<Truck>>,
    pub push_downstream_trucks: Output<(Truck, EventId)>,
    pub push_downstream_ore: Output<(IronOre, EventId)>,

    pub process_state: Option<(Duration, Truck)>,
    pub scheduled_event: Option<(MonotonicTime, ActionKey)>,
    pub process_quantity_distr: Distribution,
    pub process_time_distr: Distribution,
    pub time_to_next_event: Option<Duration>,
    next_event_index: u64,
    pub log_emitter: Output<TruckingProcessLog>,
    pub previous_check_time: MonotonicTime,
}

impl DumpingProcess {
    pub fn new() -> Self {
        DumpingProcess {
            element_name: String::new(),
            element_code: String::new(),
            element_type: String::new(),
            req_upstream_trucks: Requestor::default(),
            req_downstream_ore: Requestor::default(),
            req_downstream_trucks: Requestor::default(),
            withdraw_upstream_trucks: Requestor::default(),
            push_downstream_ore: Output::default(),
            push_downstream_trucks: Output::default(),
            process_state: None,
            scheduled_event: None,
            process_quantity_distr: Distribution::default(),
            process_time_distr: Distribution::default(),
            time_to_next_event: None,
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

impl Model for DumpingProcess {}

impl DumpingProcess {
    pub fn update_state(&mut self, mut source_event_id: EventId, cx: &mut Context<Self>) -> impl Future<Output = ()> + Send {
        async move {
            let time = cx.time();

            // Pre-update logic
            if let Some((scheduled_time, _)) = self.scheduled_event.as_ref() {
                if *scheduled_time <= cx.time() {
                    self.scheduled_event = None;
                }
            }

            // Begin main update logic

            // Resolve current dumping action if pending. Take the state - if time still remains before the event is due, we'll put it back
            match self.process_state.take() {
                Some((mut process_time_left, mut truck)) => {
                    let duration_since_prev_check = time.duration_since(self.previous_check_time);
                    process_time_left = process_time_left.saturating_sub(duration_since_prev_check);
                    if process_time_left.is_zero() {
                        
                        let ore = truck.ore.take();
                        let unwrapped_ore = ore.unwrap_or_default(); // Use empty ore if None

                        source_event_id = self.log(time, source_event_id, TruckingProcessLogType::DumpingSuccess {
                            truck_id: truck.truck_id.clone(),
                            quantity: unwrapped_ore.total(),
                            ore: unwrapped_ore.clone()
                        }).await;

                        self.push_downstream_trucks.send((truck.clone(), source_event_id.clone())).await;
                        self.push_downstream_ore.send((unwrapped_ore.clone(), source_event_id.clone())).await;
                    } else {
                        self.process_state = Some((process_time_left, truck));
                    }
                }
                None => {}
            }

            // Check if we can start a new dumping event
            match self.process_state {
                Some((time_left, _)) => {
                    self.time_to_next_event = Some(time_left);
                }
                None => {
                    let us_trucks_state = self.req_upstream_trucks.send(()).await.next();
                    let ds_ore_state = self.req_downstream_ore.send(()).await.next();
                    let ds_trucks_state = self.req_downstream_trucks.send(()).await.next();

                    match (&us_trucks_state, &ds_ore_state, &ds_trucks_state) {
                        (
                            Some(DiscreteStockState::Normal { .. }) | Some(DiscreteStockState::Full { .. }),
                            Some(VectorStockState::Empty { .. }) | Some(VectorStockState::Normal { .. }),
                            Some(DiscreteStockState::Empty { .. }) | Some(DiscreteStockState::Normal { .. }),
                        ) => {
                            source_event_id = self.log(time, source_event_id, TruckingProcessLogType::WithdrawRequest).await;
                            let mut truck = self.withdraw_upstream_trucks.send(((), source_event_id.clone())).await.next().unwrap();

                            match truck.take() {
                                Some(truck) => {

                                    let ore = match &truck.ore {
                                        Some(x) => x.clone(),
                                        None => IronOre::default(), // Zero
                                    };

                                    let process_duration = self.process_time_distr.sample();
                                    self.process_state = Some((Duration::from_secs_f64(process_duration), truck.clone()));

                                    source_event_id = self.log(time, source_event_id, TruckingProcessLogType::DumpingStart { truck_id: truck.truck_id, quantity: ore.total(), ore: ore.clone() }).await;
                                    self.time_to_next_event = Some(Duration::from_secs_f64(process_duration));
                                }
                                None => {
                                    source_event_id = self.log(time, source_event_id, TruckingProcessLogType::DumpingFailure { reason: "No truck available upstream" }).await;
                                    self.time_to_next_event = None;
                                }
                            }
                        }
                        (Some(DiscreteStockState::Empty { .. }) | None, _, _) => {
                            source_event_id = self.log(time, source_event_id, TruckingProcessLogType::DumpingFailure { reason: "Upstream truck stock is empty or not connected" }).await;
                            self.time_to_next_event = None;
                        },
                        (_, Some(VectorStockState::Full { .. }) | None, _) => {
                            source_event_id = self.log(time, source_event_id, TruckingProcessLogType::DumpingFailure { reason: "No ore can be received downstream" }).await;
                            self.time_to_next_event = None;
                        },
                        (_, _, Some(DiscreteStockState::Full { .. }) | None) => {
                            source_event_id = self.log(time, source_event_id, TruckingProcessLogType::DumpingFailure { reason: "Downstream truck stock is full or not connected" }).await;
                            self.time_to_next_event = None;
                        },
                    }
                }
            }

            // Post-update logic

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

    pub fn log(&mut self, time: MonotonicTime, source_event_id: EventId, details: TruckingProcessLogType) -> impl Future<Output = EventId> + Send {
        async move {
            let event_id = EventId(format!("{}_{:06}", self.element_code, self.next_event_index));
            let log = TruckingProcessLog {
                time: time.to_chrono_date_time(0).unwrap().to_string(),
                event_id: event_id.clone(),
                source_event_id,
                element_name: self.element_name.clone(),
                element_type: self.element_type.clone(),
                event: details,
            };
            self.log_emitter.send(log).await;
            self.next_event_index += 1;

            event_id
        }
    }
}
