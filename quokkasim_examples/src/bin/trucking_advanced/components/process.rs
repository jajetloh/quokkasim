use std::time::Duration;
use indexmap::IndexMap;
use log::warn;
use nexosim::{ports::Output, time::MonotonicTime};
use quokkasim::{core::{Distribution, NotificationMetadata}, define_combiner_process, define_process, define_splitter_process, prelude::{VectorResource, VectorStockState}};
use serde::{ser::SerializeStruct, Serialize};

use super::{stock::{TruckAndOreStockLog, TruckAndOreStockLogDetails, TruckStockState}, TruckAndOre};


#[derive(Debug, Clone)]
pub struct TruckingProcessLog {
    pub time: String,
    pub element_name: String,
    pub element_type: String,
    pub event_id: String,
    pub process_data: TruckingProcessLogType,
}

impl Serialize for TruckingProcessLog {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("TruckingProcessLog", 10)?;
        state.serialize_field("event_id", &self.event_id)?;
        state.serialize_field("time", &self.time)?;
        state.serialize_field("element_name", &self.element_name)?;
        state.serialize_field("element_type", &self.element_type)?;

        let (event_type, truck_id, total, x0, x1, x2, x3, x4, reason): (
            Option<&'static str>, Option<i32>, Option<f64>, Option<f64>, Option<f64>, Option<f64>, Option<f64>, Option<f64>, Option<&'static str>,
        ) = match &self.process_data {
            TruckingProcessLogType::LoadStart { truck_id, tonnes, components, .. } => (
                Some("LoadStart"), Some(*truck_id), Some(*tonnes), Some(components[0]), Some(components[1]), Some(components[2]), Some(components[3]), Some(components[4]), None,
            ),
            TruckingProcessLogType::LoadSuccess { truck_id, tonnes, components, .. } => (
                Some("LoadSuccess"), Some(*truck_id), Some(*tonnes), Some(components[0]), Some(components[1]), Some(components[2]), Some(components[3]), Some(components[4]), None,
            ),
            TruckingProcessLogType::LoadStartFailed { reason } => (
                Some("LoadStartFailed"), None, None, None, None, None, None, None, Some(*reason),
            ),
            TruckingProcessLogType::DumpStart { truck_id, tonnes, components, .. } => (
                Some("DumpStart"), Some(*truck_id), Some(*tonnes), Some(components[0]), Some(components[1]), Some(components[2]), Some(components[3]), Some(components[4]), None,
            ),
            TruckingProcessLogType::DumpSuccess { truck_id, tonnes, components, .. } => (
                Some("DumpSuccess"), Some(*truck_id), Some(*tonnes), Some(components[0]), Some(components[1]), Some(components[2]), Some(components[3]), Some(components[4]), None,
            ),
            TruckingProcessLogType::DumpStartFailed { reason } => ( Some("DumpStartFailed"), None, None, None, None, None, None, None, Some(*reason), ),
            TruckingProcessLogType::TruckMovement { truck_id, tonnes, components, .. } => (Some("TruckMovement"), Some(*truck_id), Some(*tonnes), Some(components[0]), Some(components[1]), Some(components[2]), Some(components[3]), Some(components[4]), None),
        };

        state.serialize_field("event_type", &event_type)?;
        state.serialize_field("truck_id", &truck_id)?;
        state.serialize_field("total", &total)?;
        state.serialize_field("x0", &x0)?;
        state.serialize_field("x1", &x1)?;
        state.serialize_field("x2", &x2)?;
        state.serialize_field("x3", &x3)?;
        state.serialize_field("x4", &x4)?;
        state.serialize_field("reason", &reason)?;
        state.end()
    }
}

#[derive(Debug, Clone)]
pub enum TruckingProcessLogType {
    LoadStart {
        truck_id: i32,
        tonnes: f64,
        components: [f64; 5],
    },
    LoadSuccess {
        truck_id: i32,
        tonnes: f64,
        components: [f64; 5],
    },
    LoadStartFailed {
        reason: &'static str,
    },
    DumpStart {
        truck_id: i32,
        tonnes: f64,
        components: [f64; 5],
    },
    DumpSuccess {
        truck_id: i32,
        tonnes: f64,
        components: [f64; 5],
    },
    DumpStartFailed {
        reason: &'static str,
    },
    TruckMovement {
        truck_id: i32,
        tonnes: f64,
        components: [f64; 5],
    },
}


#[derive(Debug, Clone)]
pub enum LoadingProcessState {
    Loading {
        truck: TruckAndOre,
        previous_check_time: MonotonicTime,
        time_until_done: Duration,
    },
    Idle,
}

impl Default for LoadingProcessState {
    fn default() -> Self {
        LoadingProcessState::Idle
    }
}

define_combiner_process!(
    /// Loading process for trucks. Draws from a truck stock and vector stock, pushes out Vec<TruckAndOre>
    name = LoadingProcess,
    inflow_stock_state_types = (VectorStockState, TruckStockState),
    resource_in_types = (VectorResource, Option<TruckAndOre>),
    resource_in_parameter_types = (f64, ()),
    outflow_stock_state_type = TruckStockState,
    resource_out_type = Option<TruckAndOre>,
    resource_out_parameter_type = Option<TruckAndOre>,
    check_update_method = |mut x: Self, time: MonotonicTime| {
        async move {
            // First resolve Loading state, if applicable
            match x.state.clone() {
                LoadingProcessState::Loading { truck, previous_check_time, time_until_done } => {
                    let elapsed_time = time.duration_since(previous_check_time);
                    let new_time_until_done = time_until_done.saturating_sub(elapsed_time);
                    let new_previous_check_time = time;

                    if new_time_until_done.is_zero() {
                        x.log(time, TruckingProcessLogType::LoadSuccess { truck_id: truck.truck,  tonnes: truck.ore.total(), components: truck.ore.vec } ).await;
                        x.log_truck_stock(time, TruckAndOreStockLogDetails::StockAdded { truck_id: truck.truck, total: truck.ore.total(), empty: 999., contents: truck.ore.vec }).await;
                        x.push_downstream.send((Some(truck.clone()), NotificationMetadata {
                            time,
                            element_from: x.element_name.clone(),
                            message: "Truck and ore".into(),
                        })).await;
                        x.state = LoadingProcessState::Idle;
                    } else {
                        x.state = LoadingProcessState::Loading { truck, previous_check_time: new_previous_check_time, time_until_done: new_time_until_done };
                        x.time_to_next_event_counter = Some(time_until_done);
                        return x;
                    }
                },
                LoadingProcessState::Idle => {}
            }

            // Then execute new load
            let us_material_state: VectorStockState = x.req_upstreams.0.send(()).await.next().unwrap();
            let us_truck_state: TruckStockState = x.req_upstreams.1.send(()).await.next().unwrap();

            match (&us_material_state, &us_truck_state) {
                (VectorStockState::Normal { .. } | VectorStockState::Full { .. }, TruckStockState::Normal { .. }) => {
                    let mut truck = x.withdraw_upstreams.1.send(((), NotificationMetadata {
                        time,
                        element_from: x.element_name.clone(),
                        message: "Truck request".into(),
                    })).await.next().unwrap();
                    let material = x.withdraw_upstreams.0.send((x.load_quantity_dist.as_mut().unwrap().sample(), NotificationMetadata {
                        time,
                        element_from: x.element_name.clone(),
                        message: "Material request".into(),
                    })).await.next().unwrap();

                    match truck.take() {
                        Some(mut truck) => {
                            let truck_id = truck.truck;
                            truck.ore = material.clone();
                            let time_until_done = Duration::from_secs_f64(x.load_time_dist_secs.as_mut().unwrap().sample());
                            x.state = LoadingProcessState::Loading { truck, previous_check_time: time.clone(), time_until_done };
                            x.log(time, TruckingProcessLogType::LoadStart { truck_id,  tonnes: material.total(), components: material.vec.clone() } ).await;
                            x.time_to_next_event_counter = Some(time_until_done);
                        },
                        None => {
                            x.state = LoadingProcessState::Idle;
                            x.log(time, TruckingProcessLogType::LoadStartFailed { reason: "No trucks available" }).await;
                            x.time_to_next_event_counter = None;
                        }
                    }
                },
                (VectorStockState::Empty { .. }, _) => {
                    x.log(time, TruckingProcessLogType::LoadStartFailed { reason: "No material available" }).await;
                    x.time_to_next_event_counter = None;
                },
                (_, TruckStockState::Empty) => {
                    x.log(time, TruckingProcessLogType::LoadStartFailed { reason: "No trucks available" }).await;
                    x.time_to_next_event_counter = None;
                }
            }
            x
        }
    },
    fields = {
        state: LoadingProcessState,
        truck_stock_emitter: Output<TruckAndOreStockLog>,
        load_time_dist_secs: Option<Distribution>,
        load_quantity_dist: Option<Distribution>
    },
    log_record_type = TruckingProcessLog,
    log_method = |x: &'a mut Self, time: MonotonicTime, details: TruckingProcessLogType| {
        async move {
            let log = TruckingProcessLog {
                time: time.to_chrono_date_time(0).unwrap().to_string(),
                element_name: x.element_name.clone(),
                element_type: x.element_type.clone(),
                event_id: x.get_event_id(),
                process_data: details,
            };
            x.log_emitter.send(log).await;
        }
    },
    log_method_parameter_type = TruckingProcessLogType
);

impl LoadingProcess {
    pub fn log_truck_stock(
        &mut self,
        time: MonotonicTime,
        details: TruckAndOreStockLogDetails,
    ) -> impl Future<Output = ()> {
        async move {
            let log: TruckAndOreStockLog = TruckAndOreStockLog {
                time: time.to_string(),
                element_name: self.element_name.clone(),
                element_type: self.element_type.clone(),
                details,
            };
            self.truck_stock_emitter.send(log).await;
        }
    }
}


#[derive(Debug, Clone)]
pub enum DumpingProcessState {
    Dumping {
        truck: TruckAndOre,
        previous_check_time: MonotonicTime,
        time_until_done: Duration,
    },
    Idle,
}

impl Default for DumpingProcessState {
    fn default() -> Self {
        DumpingProcessState::Idle
    }
}

define_splitter_process!(
    /// DumpingProcess
    name = DumpingProcess,
    inflow_stock_state_type = TruckStockState,
    resource_in_type = Option<TruckAndOre>,
    resource_in_parameter_type = (),
    outflow_stock_state_types = (VectorStockState, TruckStockState),
    resource_out_types = (ArrayResource, Option<TruckAndOre>),
    resource_out_parameter_types = (VectorResource, Option<TruckAndOre>),
    check_update_method = |mut x: Self, time: MonotonicTime| {
        async move {
            // Resolve Dumping state, if applicable
            match x.state.clone() {
                DumpingProcessState::Dumping { truck, previous_check_time, time_until_done } => {
                    let elapsed_time = time.duration_since(previous_check_time);
                    let new_time_until_done = time_until_done.saturating_sub(elapsed_time);
                    let new_previous_check_time = time;

                    if new_time_until_done.is_zero() {
                        x.log(time, TruckingProcessLogType::DumpSuccess { truck_id: truck.truck, tonnes: truck.ore.total(), components: truck.ore.vec } ).await;
                        x.log_truck_stock(time, TruckAndOreStockLogDetails::StockRemoved { truck_id: truck.truck, total: truck.ore.total(), empty: 999., contents: truck.ore.vec }).await;
                        x.push_downstreams.1.send((Some(truck.clone()), NotificationMetadata {
                            time,
                            element_from: x.element_name.clone(),
                            message: "Truck done".into(),
                        })).await;
                        x.push_downstreams.0.send((truck.ore.clone(), NotificationMetadata {
                            time,
                            element_from: x.element_name.clone(),
                            message: "Material request".into(),
                        })).await;
                        x.state = DumpingProcessState::Idle;
                    } else {
                        x.state = DumpingProcessState::Dumping { truck, previous_check_time: new_previous_check_time, time_until_done: new_time_until_done };
                        x.time_to_next_event_counter = Some(time_until_done);
                        return x;
                    }
                },
                DumpingProcessState::Idle => {}
            }

            let us_state: TruckStockState = x.req_upstream.send(()).await.next().unwrap();
            let ds_material_state: VectorStockState = x.req_downstreams.0.send(()).await.next().unwrap();
            match (us_state, ds_material_state) {
                (TruckStockState::Normal { .. }, VectorStockState::Normal { .. } | VectorStockState::Empty { .. }) => {
                    let truck_and_ore: Option<TruckAndOre> = x.withdraw_upstream.send(((), NotificationMetadata {
                        time,
                        element_from: x.element_name.clone(),
                        message: "Truck request".into(),
                    })).await.next().unwrap();

                    match truck_and_ore {
                        Some(truck_and_ore) => {
                            let time_until_done = Duration::from_secs_f64(x.dump_time_dist_secs.as_mut().unwrap().sample());
                            x.state = DumpingProcessState::Dumping {
                                truck: truck_and_ore.clone(),
                                previous_check_time: time.clone(),
                                time_until_done,
                            };
                            x.log(time, TruckingProcessLogType::DumpStart { truck_id: truck_and_ore.truck, tonnes: truck_and_ore.ore.total(), components: truck_and_ore.ore.vec } ).await;
                            x.time_to_next_event_counter = Some(time_until_done);
                        },
                        None => {
                            x.state = DumpingProcessState::Idle;
                            x.log(time, TruckingProcessLogType::DumpStartFailed { reason: "No trucks available" }).await;
                            x.time_to_next_event_counter = None;
                            return x;
                        }
                    }
                },
                (TruckStockState::Empty, _) => {
                    x.log(time, TruckingProcessLogType::DumpStartFailed { reason: "No trucks available" }).await;
                    x.time_to_next_event_counter = None;
                },
                (_, VectorStockState::Full { .. }) => {
                    x.log(time, TruckingProcessLogType::DumpStartFailed { reason: "Downstream material stock is full" }).await;
                    x.time_to_next_event_counter = None;
                },
            }
            x
        }
    },
    fields = {
        state: DumpingProcessState,
        truck_stock_emitter: Output<TruckAndOreStockLog>,
        dump_time_dist_secs: Option<Distribution>
    },
    log_record_type = TruckingProcessLog,
    log_method = |x: &'a mut Self, time: MonotonicTime, details: TruckingProcessLogType| {
        async move {
            let log = TruckingProcessLog {
                time: time.to_chrono_date_time(0).unwrap().to_string(),
                element_name: x.element_name.clone(),
                element_type: x.element_type.clone(),
                event_id: x.get_event_id(),
                process_data: details,
            };
            x.log_emitter.send(log).await;
        }
    },
    log_method_parameter_type = TruckingProcessLogType
);

impl DumpingProcess {
    pub fn log_truck_stock(
        &mut self,
        time: MonotonicTime,
        details: TruckAndOreStockLogDetails,
    ) -> impl Future<Output = ()> {
        async move {
            let log: TruckAndOreStockLog = TruckAndOreStockLog {
                time: time.to_string(),
                element_name: self.element_name.clone(),
                element_type: self.element_type.clone(),
                details,
            };
            self.truck_stock_emitter.send(log).await;
        }
    }
}


define_process!(
    /// TruckMovementProcess
    name = TruckMovementProcess,
    stock_state_type = TruckStockState,
    resource_in_type = Option<TruckAndOre>,
    resource_in_parameter_type = i32,
    resource_out_type = Option<TruckAndOre>,
    resource_out_parameter_type = Option<TruckAndOre>,
    check_update_method = |mut x: Self, time: MonotonicTime| {
        async move {
            let elapsed_time: Duration = match x.previous_check_time {
                None => Duration::MAX,
                Some(t) => time.duration_since(t),
            };

            let mut items_ready: Vec<i32> = vec![];
            x.time_to_next_event_counter = None;
            for (id, counter) in x.time_counters.iter_mut() {
                *counter = counter.saturating_sub(elapsed_time);
                if counter.is_zero() {
                    items_ready.push(*id);
                } else {
                    match x.time_to_next_event_counter {
                        None => x.time_to_next_event_counter = Some(*counter),
                        Some(y) => {
                            x.time_to_next_event_counter = Some(y.min(*counter));
                        },
                    }
                }
            }

            // Check for trucks that are done
            for id in items_ready {
                x.time_counters.swap_remove(&id);
                let truck_and_ore: Option<TruckAndOre> = x.withdraw_upstream.send((id, NotificationMetadata {
                    time,
                    element_from: x.element_name.clone(),
                    message: "Truck request".into(),
                })).await.next().unwrap();
                match truck_and_ore {
                    Some(truck_and_ore) => {
                        x.log(time, TruckingProcessLogType::TruckMovement { truck_id: id, tonnes: truck_and_ore.ore.total(), components: truck_and_ore.ore.vec } ).await;
                        x.push_downstream.send((Some(truck_and_ore), NotificationMetadata {
                            time,
                            element_from: x.element_name.clone(),
                            message: "Truck and ore".into(),
                        })).await;
                    },
                    None => {
                        x.log(time, TruckingProcessLogType::LoadStartFailed { reason: "No truck with requested id exists upstream" }).await;
                        warn!("TruckMovementProcess {}: No truck with requested id {} exists upstream", x.element_name, id);
                        x.time_to_next_event_counter = None;
                        return x;
                    }
                }
            }

            // Check for new trucks upstream. If new, add a counter for it
            let us_state: TruckStockState = x.req_upstream.send(()).await.next().unwrap();

            match us_state {
                TruckStockState::Normal(y) => {
                    for id in y.iter() {
                        if !x.time_counters.contains_key(id) {
                            let travel_time = Duration::from_secs_f64(x.travel_time_dist_secs.as_mut().unwrap_or_else(|| panic!("travel_time_dist_secs not set for {}", x.element_name)).sample());
                            x.time_counters.insert(*id, travel_time);
                            match x.time_to_next_event_counter {
                                None => x.time_to_next_event_counter = Some(travel_time),
                                Some(y) => {
                                    x.time_to_next_event_counter = Some(y.min(travel_time));
                                }
                            }
                        }
                    }
                },
                TruckStockState::Empty => {}
            }
            x
        }
    },
    fields = {
        time_counters: IndexMap<i32, Duration>,
        travel_time_dist_secs: Option<Distribution>
    },
    log_record_type = TruckingProcessLog,
    log_method = |x: &'a mut Self, time: MonotonicTime, details: TruckingProcessLogType| {
        async move {
            let log = TruckingProcessLog {
                time: time.to_chrono_date_time(0).unwrap().to_string(),
                element_name: x.element_name.clone(),
                element_type: x.element_type.clone(),
                event_id: x.get_event_id(),
                process_data: details,
            };
            x.log_emitter.send(log).await;
        }
    },
    log_method_parameter_type = TruckingProcessLogType
);
