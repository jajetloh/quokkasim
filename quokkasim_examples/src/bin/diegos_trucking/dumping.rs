use std::time::Duration;

use quokkasim::prelude::*;

use crate::iron_ore::IronOre;
use crate::loggers::{TruckingProcessLog, TruckingProcessLogType};
use crate::truck::Truck;

pub struct DumpingProcess {
    pub element_name: String,
    pub element_type: String,
    pub req_upstream_trucks: Requestor<(), DiscreteStockState>,
    pub req_downstream_trucks: Requestor<(), DiscreteStockState>,
    pub req_downstream_ore: Requestor<(), VectorStockState>,

    pub withdraw_upstream_trucks: Requestor<((), NotificationMetadata), Option<Truck>>,
    pub push_downstream_trucks: Output<(Option<Truck>, NotificationMetadata)>,
    pub push_downstream_ore: Output<(IronOre, NotificationMetadata)>,

    pub process_state: Option<(Duration, Truck)>,
    pub process_quantity_distr: Distribution,
    pub process_time_distr: Distribution,
    pub time_to_next_event: Option<Duration>,
    next_event_id: u64,
    pub log_emitter: Output<TruckingProcessLog>,
    pub previous_check_time: MonotonicTime,
}

// Utility methods
impl DumpingProcess {
    pub fn new() -> Self {
        DumpingProcess {
            element_name: String::new(),
            element_type: String::new(),
            req_upstream_trucks: Requestor::default(),
            req_downstream_ore: Requestor::default(),
            req_downstream_trucks: Requestor::default(),
            withdraw_upstream_trucks: Requestor::default(),
            push_downstream_ore: Output::default(),
            push_downstream_trucks: Output::default(),
            process_state: None,
            process_quantity_distr: Distribution::default(),
            process_time_distr: Distribution::default(),
            time_to_next_event: None,
            next_event_id: 0,
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
    pub fn update_state<'a> (&'a mut self, notif_meta: NotificationMetadata, cx: &'a mut Context<Self>) -> impl Future<Output = ()> + Send + 'a {
        async move {
            let time = cx.time();

            // Resolve current dumping action if pending
            match self.process_state.take() {

                // we only have the truck stock upstream
                Some((mut process_time_left, mut truck)) => {
                    let duration_since_prev_check = time.duration_since(self.previous_check_time);
                    process_time_left = process_time_left.saturating_sub(duration_since_prev_check);
                    if process_time_left.is_zero() {
                        let ore = truck.ore.take();
                        match ore {
                            Some(ore) => {
                                self.push_downstream_trucks.send((Some(truck.clone()), NotificationMetadata {
                                    time,
                                    element_from: self.element_name.clone(),
                                    message: "Dumped truck without ore".into(),
                                })).await;

                                // send ore to ore stock - silly question, do we send all of it "at once"
                                // This bit runs at the after the dumping duration is passed, so yes at this point we send it all
                                self.push_downstream_ore.send((ore.clone(), NotificationMetadata {
                                    time,
                                    element_from: self.element_name.clone(),
                                    message: "Ore received at destination stock".into(),
                                })).await;
                                self.log(time, TruckingProcessLogType::DumpingSuccess { truck_id: truck.truck_id.clone(), quantity: ore.total(), ore: ore.clone() }).await;

                            },
                            None => {
                                panic!("Hold on a second! Truck has no ore to dump!"); // no ore to dump
                            }
                        }
                        // Don't need this part as truck.take() before already cleaned truck.ore up
                        // match truck.ore {
                        //     Some(_) => {
                        //         truck.ore = None; // dumpin'
                        //     },
                        //     None => {
                        //         panic!("Hold on a second! Truck already dumped, cannot dump again!"); // dumped already
                        //     }
                        // }
                        
                        // send empty truck to truck stock

                    } else {
                        self.process_state = Some((process_time_left, truck));
                    }
                }
                None => {}
            }

            // Check if we need to start a new dumping process
            match self.process_state {
                Some(_) => {
                    // Already dumpin', nothing to do
                }
                None => {
                    // Check if we have trucks available for dumpin'
                    let us_trucks_state = self.req_upstream_trucks.send(()).await.next();
                    let ds_ore_state = self.req_downstream_ore.send(()).await.next();
                    let ds_trucks_state = self.req_downstream_trucks.send(()).await.next();

                    match (&us_trucks_state, &ds_ore_state, &ds_trucks_state) {
                        // adjusted vector stock so it's either empty or normal - if it's full we can't add more to it
                        (
                            Some(DiscreteStockState::Normal { .. }) | Some(DiscreteStockState::Full { .. }),
                            Some(VectorStockState::Empty { .. }) | Some(VectorStockState::Normal { .. }),
                            Some(DiscreteStockState::Empty { .. }) | Some(DiscreteStockState::Normal { .. }),
                        ) => {

                            let mut truck = self.withdraw_upstream_trucks.send(((), NotificationMetadata {
                                time,
                                element_from: self.element_name.clone(),
                                message: "Requesting truck for dumping".into(),
                            })).await.next().unwrap();

                            match truck.take() {
                                Some(Truck { ore: None, .. }) => {
                                    // Truck already dumped - how did we get here? No idea.
                                    panic!("Truck has already been dumped, cannot dump again");
                                },
                                Some(truck) => {
                                    // There's some ore in the truck - woop woop - let's DUMP
                                    // let process_quantity = self.process_quantity_distr.sample();
                                    let ore = truck.ore.clone().unwrap();

                                    // No need to send upstream withdraw request 

                                    // let ore = self.withdraw_upstream_ore.send((process_quantity, NotificationMetadata {
                                    //     time,
                                    //     element_from: self.element_name.clone(),
                                    //     message: "Requesting ore for loading".into(),
                                    // })).await.next().unwrap();


                                    // not sure if this is the correct way to manage dumping
                                    let process_duration = self.process_time_distr.sample();
                                    self.process_state = Some((Duration::from_secs_f64(process_duration), truck.clone()));

                                    self.log(time, TruckingProcessLogType::DumpingStart { truck_id: truck.truck_id, quantity: ore.total(), ore: ore.clone() }).await;
                                    self.time_to_next_event = Some(Duration::from_secs_f64(process_duration)); // Retry after 1 second
                                }
                                None => {
                                    // No truck available upstream
                                    self.log(time, TruckingProcessLogType::DumpingFailure { reason: "No truck available upstream" }).await;
                                    self.time_to_next_event = None;
                                }
                            }
                        }

                        

                        (Some(DiscreteStockState::Empty { .. }) | None, _, _) => {
                            // Upstream truck stock is empty
                            self.log(time, TruckingProcessLogType::DumpingFailure { reason: "Upstream truck stock is empty or not connected" }).await;
                            self.time_to_next_event = None;
                        },

                        // adjusted to case where downstream stock is full
                        (_, Some(VectorStockState::Full { .. }) | None, _) => {
                            // No ore can be received downstream
                            self.log(time, TruckingProcessLogType::DumpingFailure { reason: "No ore can be received downstream" }).await;
                            self.time_to_next_event = None;
                        },

                        (_, _, Some(DiscreteStockState::Full { .. }) | None) => {
                            // Downstream truck stock is full
                            self.log(time, TruckingProcessLogType::DumpingFailure { reason: "Downstream truck stock is full or not connected" }).await;
                            self.time_to_next_event = None;
                        },
                    }

                    self.previous_check_time = time;
                    match self.time_to_next_event {
                        Some(time_until_next) if time_until_next > Duration::ZERO => {
                            let next_time = time + time_until_next;
                            let notif_meta = NotificationMetadata {
                                time: next_time,
                                element_from: self.element_name.clone(),
                                message: "Scheduling next dumping process check".into(),
                            };
                            cx.schedule_event(next_time, Self::update_state, notif_meta);
                        }
                        _ => {
                            // No further events scheduled
                            self.time_to_next_event = None;
                        }
                    }
                }
            }
        }

    }

    pub fn log(&mut self, time: MonotonicTime, log_type: TruckingProcessLogType) -> impl Future<Output = ()> + Send {
        async move {
            let log = TruckingProcessLog {
                time: time.to_chrono_date_time(0).unwrap().to_string(),
                event_id: self.next_event_id,
                element_name: self.element_name.clone(),
                element_type: self.element_type.clone(),
                event: log_type,
            };
            self.log_emitter.send(log).await;
            self.next_event_id += 1;
        }
    }
}
