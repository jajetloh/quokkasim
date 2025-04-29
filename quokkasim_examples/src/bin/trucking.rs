use clap::Parser;
use csv::WriterBuilder;
use indexmap::{IndexMap, IndexSet};
use nexosim::{
    model::{Context, Model}, ports::{EventBuffer, Output, Requestor}, simulation::Address, time::MonotonicTime
};
use quokkasim::{
    // common::EventLogger,
    components::array::{ArrayResource, ArrayStock, ArrayStockLog, ArrayStockState},
    core::{
        Distribution, DistributionConfig, DistributionFactory, Mailbox, ResourceAdd,
        ResourceRemove, SimInit, StateEq,
    },
    define_combiner_process, define_process, define_splitter_process, define_stock,
    prelude::QueueStockLog,
};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use serde::{ser::SerializeStruct, Deserialize, Serialize};
use std::{error::Error, fs::File, io::BufReader, time::Duration};
use log::warn;

#[derive(Debug, Clone)]
pub struct TruckAndOre {
    truck: i32,
    ore: ArrayResource,
}

pub struct TruckAndOreMap {
    trucks: IndexMap<i32, TruckAndOre>,
}

impl ResourceAdd<Option<TruckAndOre>> for TruckAndOreMap {
    fn add(&mut self, truck_and_ore: Option<TruckAndOre>) {
        match truck_and_ore {
            Some(item) => {
                self.trucks.insert(item.truck, item);
            }
            None => {}
        }
    }
}

impl ResourceRemove<i32, Option<TruckAndOre>> for TruckAndOreMap {
    fn sub(&mut self, id: i32) -> Option<TruckAndOre> {
        self.trucks.swap_remove(&id)
    }
}

impl Default for TruckAndOreMap {
    fn default() -> Self {
        TruckAndOreMap {
            trucks: IndexMap::new(),
        }
    }
}

#[derive(Debug, Clone)]
struct TruckingProcessLog {
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
enum TruckingProcessLogType {
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
enum LoadingProcessState {
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
    /// Loading process for trucks. Draws from a truck stock and array stock, pushes out Vec<TruckAndOre>
    name = LoadingProcess,
    inflow_stock_state_types = (ArrayStockState, TruckStockState),
    resource_in_types = (ArrayResource, Option<TruckAndOre>),
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
            let us_material_state: ArrayStockState = x.req_upstreams.0.send(()).await.next().unwrap();
            let us_truck_state: TruckStockState = x.req_upstreams.1.send(()).await.next().unwrap();

            match (&us_material_state, &us_truck_state) {
                (ArrayStockState::Normal { .. } | ArrayStockState::Full { .. }, TruckStockState::Normal { .. }) => {
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
                (ArrayStockState::Empty { .. }, _) => {
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

            println!("TruckMovementProcess {}: {:?}, time_counters {:?}", x.element_name, time.to_string(), x.time_counters);

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

            println!("TruckMovementProcess {}: {:?}, us_state {:?}", x.element_name, time.to_string(), us_state);

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
    outflow_stock_state_types = (ArrayStockState, TruckStockState),
    resource_out_types = (ArrayResource, Option<TruckAndOre>),
    resource_out_parameter_types = (ArrayResource, Option<TruckAndOre>),
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
            let ds_material_state: ArrayStockState = x.req_downstreams.0.send(()).await.next().unwrap();
            match (us_state, ds_material_state) {
                (TruckStockState::Normal { .. }, ArrayStockState::Normal { .. } | ArrayStockState::Empty { .. }) => {
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
                (_, ArrayStockState::Full { .. }) => {
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

#[derive(Debug, Clone)]
pub enum TruckStockState {
    Empty,
    Normal(IndexSet<i32>),
}

impl StateEq for TruckStockState {
    fn is_same_state(&self, other: &Self) -> bool {
        match (self, other) {
            (TruckStockState::Empty, TruckStockState::Empty) => true,
            (TruckStockState::Normal(x), TruckStockState::Normal(y)) => x == y,
            _ => false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TruckAndOreStockLog {
    pub time: String,
    pub element_name: String,
    pub element_type: String,
    pub details: TruckAndOreStockLogDetails,
}

impl Serialize for TruckAndOreStockLog {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("TruckAndOreStockLog", 10)?;
        state.serialize_field("time", &self.time)?;
        state.serialize_field("element_name", &self.element_name)?;
        state.serialize_field("element_type", &self.element_type)?;
        let (log_type, truck_id, occupied, empty, x0, x1, x2, x3, x4): (
            &str, i32, f64, f64, f64, f64, f64, f64, f64,
        ) = match self.details {
            TruckAndOreStockLogDetails::StockAdded { truck_id, total, empty, contents,} => (
                "StockAdded".into(), truck_id, total, empty, contents[0], contents[1], contents[2], contents[3], contents[4],
            ),
            TruckAndOreStockLogDetails::StockRemoved { truck_id, total, empty, contents,} => (
                "StockRemoved".into(), truck_id, total, empty, contents[0], contents[1], contents[2], contents[3], contents[4],
            ),
        };
        state.serialize_field("event_type", &log_type)?;
        state.serialize_field("truck_id", &truck_id)?;
        state.serialize_field("occupied", &occupied)?;
        state.serialize_field("empty", &empty)?;
        state.serialize_field("x0", &x0)?;
        state.serialize_field("x1", &x1)?;
        state.serialize_field("x2", &x2)?;
        state.serialize_field("x3", &x3)?;
        state.serialize_field("x4", &x4)?;
        state.end()
    }
}

#[derive(Debug, Clone)]
pub enum TruckAndOreStockLogDetails {
    StockAdded {
        truck_id: i32,
        total: f64,
        empty: f64,
        contents: [f64; 5],
    },
    StockRemoved {
        truck_id: i32,
        total: f64,
        empty: f64,
        contents: [f64; 5],
    },
}

define_stock!(
    /// TruckStock
    name = TruckStock,
    resource_type = TruckAndOreMap,
    initial_resource = Default::default(),
    add_type = Option<TruckAndOre>,
    remove_type = Option<TruckAndOre>,
    remove_parameter_type = i32,
    state_type = TruckStockState,
    fields = {
        low_capacity: f64,
        max_capacity: f64,
        remaining_durations: IndexMap<i32, Duration>
    },
    get_state_method = |x: &Self| -> TruckStockState {
        if x.resource.trucks.is_empty() {
            TruckStockState::Empty
        } else {
            TruckStockState::Normal(IndexSet::from_iter(x.resource.trucks.clone().into_keys()))
        }
    },
    check_update_method = |x: &mut Self, cx: &mut Context<Self>| {
    },
    log_record_type = QueueStockLog,
    log_method = |x: &'a mut Self, time: MonotonicTime, log_type: String| {
        async move {
            let log = QueueStockLog {
                time: time.to_chrono_date_time(0).unwrap().to_string(),
                element_name: x.element_name.clone(),
                element_type: x.element_type.clone(),
                log_type,
                occupied: x.resource.trucks.len() as i32,
                empty: 999,
                state: "".into(),
                contents: "".into(),
            };
            x.log_emitter.send(log).await;
        }
    }
);

impl TruckStock {
    pub fn remove_any(
        &mut self,
        data: ((), NotificationMetadata),
        cx: &mut Context<Self>,
    ) -> impl Future<Output = Option<TruckAndOre>> {
        async move {
            self.prev_state = Some(self.get_state().await);
            let truck = match self.resource.trucks.pop() {
                Some((_, truck)) => Some(truck),
                None => {
                    None
                }
            };
            self.log(data.1.time, "SomeLog".into()).await;
            self.check_update_state(data.1, cx).await;
            truck
        }
    }
}

/// Trucking simulation command line options.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The base seed used for random distributions.
    #[arg(long, default_value = "1")]
    seed: String,

    /// The number of trucks to simulate.
    #[arg(long, default_value = "2")]
    num_trucks: usize,

    /// The simulation duration in seconds.
    #[arg(long, default_value = "21600")]
    sim_duration_secs: f64,
}

struct ParsedArgs {
    seed: u64,
    num_trucks: usize,
    sim_duration_secs: f64,
}

/// Parse a seed string such as "0..6" or "0..=7" into a vector of u64 values.
fn parse_seed_range(seed_str: &str) -> Result<Vec<u64>, String> {
    if let Some(idx) = seed_str.find("..=") {
        let (start, end) = seed_str.split_at(idx);
        let end = &end[3..]; // skip "..="
        let start: u64 = start
            .trim()
            .parse()
            .map_err(|e| format!("Invalid start seed '{}': {}", start.trim(), e))?;
        let end: u64 = end
            .trim()
            .parse()
            .map_err(|e| format!("Invalid end seed '{}': {}", end.trim(), e))?;
        Ok((start..=end).collect())
    } else if let Some(idx) = seed_str.find("..") {
        let (start, end) = seed_str.split_at(idx);
        let end = &end[2..]; // skip ".."
        let start: u64 = start
            .trim()
            .parse()
            .map_err(|e| format!("Invalid start seed '{}': {}", start.trim(), e))?;
        let end: u64 = end
            .trim()
            .parse()
            .map_err(|e| format!("Invalid end seed '{}': {}", end.trim(), e))?;
        Ok((start..end).collect())
    } else {
        seed_str
            .trim()
            .parse::<u64>()
            .map(|v| vec![v])
            .map_err(|e| format!("Invalid seed value '{}': {}", seed_str, e))
    }
}



#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
enum ComponentConfig {
    ArrayStock(ArrayStockConfig),
    TruckStock(TruckStockConfig),
    LoadingProcess(LoadingProcessConfig),
    DumpingProcess(DumpingProcessConfig),
    TruckMovementProcess(TruckMovementProcessConfig),
}

enum ComponentModel {
    ArrayStock(ArrayStock, Mailbox<ArrayStock>, Address<ArrayStock>),
    TruckStock(TruckStock, Mailbox<TruckStock>, Address<TruckStock>),
    LoadingProcess(LoadingProcess, Mailbox<LoadingProcess>, Address<LoadingProcess>),
    DumpingProcess(DumpingProcess, Mailbox<DumpingProcess>, Address<DumpingProcess>),
    TruckMovementProcess(TruckMovementProcess, Mailbox<TruckMovementProcess>, Address<TruckMovementProcess>),
}

impl ComponentModel {
    fn get_name(&self) -> &String {
        match self {
            ComponentModel::ArrayStock(x, _, _) => &x.element_name,
            ComponentModel::TruckStock(x, _, _) => &x.element_name,
            ComponentModel::LoadingProcess(x, _, _) => &x.element_name,
            ComponentModel::DumpingProcess(x, _, _) => &x.element_name,
            ComponentModel::TruckMovementProcess(x, _, _) => &x.element_name,
        }
    }
}

enum ComponentModelAddress {
    ArrayStock(Address<ArrayStock>),
    TruckStock(Address<TruckStock>),
    LoadingProcess(Address<LoadingProcess>),
    DumpingProcess(Address<DumpingProcess>),
    TruckMovementProcess(Address<TruckMovementProcess>),
}

trait CreateComponent {
    fn create_component(&self, df: &mut DistributionFactory, loggers: &mut IndexMap<String, EventLogger>) -> Result<ComponentModel, Box<dyn Error>>;
    // fn get_name(&self) -> &String;
}

impl CreateComponent for ComponentConfig {
    fn create_component(&self, df: &mut DistributionFactory, loggers: &mut IndexMap<String, EventLogger>) -> Result<ComponentModel, Box<dyn Error>> {
        match self {
            ComponentConfig::ArrayStock(config) => config.create_component(df, loggers),
            ComponentConfig::TruckStock(config) => config.create_component(df, loggers),
            ComponentConfig::LoadingProcess(config) => config.create_component(df, loggers),
            ComponentConfig::DumpingProcess(config) => config.create_component(df, loggers),
            ComponentConfig::TruckMovementProcess(config) => config.create_component(df, loggers),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct ArrayStockConfig {
    name: String,
    vec: [f64; 5],
    low_capacity: f64,
    max_capacity: f64,
    loggers: Vec<String>,
}

impl ArrayStockConfig {
    fn create_component(&self, df: &mut DistributionFactory, loggers: &mut IndexMap<String, EventLogger>) -> Result<ComponentModel, Box<dyn Error>> {
        let mut stock = ArrayStock::new()
            .with_name(self.name.clone());
        stock.resource.add(ArrayResource { vec: self.vec });
        stock.low_capacity = self.low_capacity;
        stock.max_capacity = self.max_capacity;
        self.loggers.iter().for_each(|logger_name| {
            match loggers.get(logger_name) {
                Some(EventLogger::ArrayStockLogger(logger)) => stock.log_emitter.connect_sink(logger.get_buffer()),
                _ => {
                    // TODO: Better error handling here
                    warn!("No logger called {} found for ArrayStock {}", logger_name, self.name);
                }
            }
        });
        let mbox = Mailbox::new();
        let addr = mbox.address();
        Ok(ComponentModel::ArrayStock(stock, mbox, addr))
    }
}

#[derive(Debug, Clone, Deserialize)]
struct TruckStockConfig {
    name: String,
    loggers: Vec<String>,
}

impl TruckStockConfig {
    fn create_component(&self, df: &mut DistributionFactory, loggers: &mut IndexMap<String, EventLogger>) -> Result<ComponentModel, Box<dyn Error>> {
        let mut stock = TruckStock::new()
            .with_name(self.name.clone());
        self.loggers.iter().for_each(|logger_name| {
            match loggers.get(logger_name) {
                Some(EventLogger::QueueStockLogger(logger)) => stock.log_emitter.connect_sink(logger.get_buffer()),
                _ => {
                    warn!("No logger called {} found for TruckStock {}", logger_name, self.name);
                }
            }
        });
        let mbox = Mailbox::new();
        let addr = mbox.address();
        Ok(ComponentModel::TruckStock(stock, mbox, addr))
    }
}

#[derive(Debug, Clone, Deserialize)]
struct LoadingProcessConfig {
    name: String,
    load_time_dist_secs: DistributionConfig,
    load_quantity_dist: DistributionConfig,
    loggers: Vec<String>,
}

impl LoadingProcessConfig {
    fn create_component(&self, df: &mut DistributionFactory, loggers: &mut IndexMap<String, EventLogger>) -> Result<ComponentModel, Box<dyn Error>> {
        let mut loading = LoadingProcess::new()
            .with_name(self.name.clone())
            .with_load_time_dist_secs(Some(df.create(self.load_time_dist_secs.clone())?))
            .with_load_quantity_dist(Some(df.create(self.load_quantity_dist.clone())?));

        self.loggers.iter().for_each(|logger_name| {
            match loggers.get(logger_name) {
                Some(EventLogger::TruckingProcessLogger(logger)) => loading.log_emitter.connect_sink(logger.get_buffer()),
                _ => {
                    warn!("No logger called {} found for LoadingProcess {}", logger_name, self.name);
                }
            }
        });

        let mbox = Mailbox::new();
        let addr = mbox.address();
        Ok(ComponentModel::LoadingProcess(loading, mbox, addr))
    }
}

#[derive(Debug, Clone, Deserialize)]
struct DumpingProcessConfig {
    name: String,
    dump_time_dist_secs: DistributionConfig,
    loggers: Vec<String>,
}

impl DumpingProcessConfig {
    fn create_component(&self, df: &mut DistributionFactory, loggers: &mut IndexMap<String, EventLogger>) -> Result<ComponentModel, Box<dyn Error>> {
        let mut dumping = DumpingProcess::new()
            .with_name(self.name.clone())
            .with_dump_time_dist_secs(Some(df.create(self.dump_time_dist_secs.clone())?));

        self.loggers.iter().for_each(|logger_name| {
            match loggers.get(logger_name) {
                Some(EventLogger::TruckingProcessLogger(logger)) => dumping.log_emitter.connect_sink(logger.get_buffer()),
                _ => {
                    warn!("No logger called {} found for DumpingProcess {}", logger_name, self.name);
                }
            }
        });

        let mbox = Mailbox::new();
        let addr = mbox.address();
        Ok(ComponentModel::DumpingProcess(dumping, mbox, addr))
    }
}

#[derive(Debug, Clone, Deserialize)]
struct TruckMovementProcessConfig {
    name: String,
    travel_time_dist_secs: DistributionConfig,
}

impl TruckMovementProcessConfig {
    fn create_component(&self, df: &mut DistributionFactory, loggers: &mut IndexMap<String, EventLogger>) -> Result<ComponentModel, Box<dyn Error>> {
        let movement = TruckMovementProcess::new()
            .with_name(self.name.clone())
            .with_travel_time_dist_secs(Some(df.create(self.travel_time_dist_secs.clone())?));
        let mbox = Mailbox::new();
        let addr = mbox.address();
        Ok(ComponentModel::TruckMovementProcess(movement, mbox, addr))
    }
}

#[derive(Debug, Clone, Deserialize)]
struct ConnectionConfig {
    upstream: String,
    downstream: String,
}

fn connect_components(
    comp1: ComponentModel,
    comp2: ComponentModel,
) -> Result<(ComponentModel, ComponentModel), Box<dyn Error>> {
    let comp1_name = comp1.get_name().clone();
    let comp2_name = comp2.get_name().clone();
    match (comp1, comp2) {
        (ComponentModel::TruckStock(mut stock_model, stock_mbox, stock_addr), ComponentModel::LoadingProcess(mut loading, loading_mbox, loading_addr)) => {
            loading.req_upstreams.1.connect(TruckStock::get_state, &stock_addr);
            loading.withdraw_upstreams.1.connect(TruckStock::remove_any, &stock_addr);
            stock_model.state_emitter.connect(LoadingProcess::check_update_state, &loading_addr);
            Ok((ComponentModel::TruckStock(stock_model, stock_mbox, stock_addr),
            ComponentModel::LoadingProcess(loading, loading_mbox, loading_addr)))
        },
        (ComponentModel::ArrayStock(mut stock_model, stock_mbox, stock_addr), ComponentModel::LoadingProcess(mut loading, loading_mbox, loading_addr)) => {
            loading.req_upstreams.0.connect(ArrayStock::get_state, &stock_addr);
            loading.withdraw_upstreams.0.connect(ArrayStock::remove, &stock_addr);
            stock_model.state_emitter.connect(LoadingProcess::check_update_state, &loading_addr);
            Ok((ComponentModel::ArrayStock(stock_model, stock_mbox, stock_addr),
            ComponentModel::LoadingProcess(loading, loading_mbox, loading_addr)))
        },
        (ComponentModel::LoadingProcess(mut loading, loading_mbox, loading_addr), ComponentModel::TruckStock(mut stock_model, stock_mbox, stock_addr)) => {
            loading.req_downstream.connect(TruckStock::get_state, &stock_addr);
            loading.push_downstream.connect(TruckStock::add, &stock_addr);
            stock_model.state_emitter.connect(LoadingProcess::check_update_state, &loading_addr);
            Ok((ComponentModel::LoadingProcess(loading, loading_mbox, loading_addr),
            ComponentModel::TruckStock(stock_model, stock_mbox, stock_addr)))
        },
        (ComponentModel::TruckStock(mut stock_model, stock_mbox, stock_addr), ComponentModel::TruckMovementProcess(mut movement, movement_mbox, movement_addr)) => {
            println!("Connecting TruckStock to TruckMovementProcess: {} to {}", stock_model.element_name, movement.element_name);
            movement.req_upstream.connect(TruckStock::get_state, &stock_addr);
            movement.withdraw_upstream.connect(TruckStock::remove, &stock_addr); 
            stock_model.state_emitter.connect(TruckMovementProcess::check_update_state, &movement_addr);
            Ok((ComponentModel::TruckStock(stock_model, stock_mbox, stock_addr),
            ComponentModel::TruckMovementProcess(movement, movement_mbox, movement_addr)))
        },
        (ComponentModel::TruckMovementProcess(mut movement, movement_mbox, movement_addr), ComponentModel::TruckStock(mut stock_model, stock_mbox, stock_addr)) => {
            movement.req_downstream.connect(TruckStock::get_state, &stock_addr);
            movement.push_downstream.connect(TruckStock::add, &stock_addr);
            stock_model.state_emitter.connect(TruckMovementProcess::check_update_state, &movement_addr);
            Ok((ComponentModel::TruckMovementProcess(movement, movement_mbox, movement_addr),
            ComponentModel::TruckStock(stock_model, stock_mbox, stock_addr)))
        },
        (ComponentModel::TruckStock(mut stock_model, stock_mbox, stock_addr), ComponentModel::DumpingProcess(mut dumping, dumping_mbox, dumping_addr)) => {
            dumping.req_upstream.connect(TruckStock::get_state, &stock_addr);
            dumping.withdraw_upstream.connect(TruckStock::remove_any, &stock_addr);
            stock_model.state_emitter.connect(DumpingProcess::check_update_state, &dumping_addr);
            Ok((ComponentModel::TruckStock(stock_model, stock_mbox, stock_addr),
            ComponentModel::DumpingProcess(dumping, dumping_mbox, dumping_addr)))
        },
        (ComponentModel::DumpingProcess(mut dumping, dumping_mbox, dumping_addr), ComponentModel::ArrayStock(mut stock_model, stock_mbox, stock_addr)) => {
            dumping.req_downstreams.0.connect(ArrayStock::get_state, &stock_addr);
            dumping.push_downstreams.0.connect(ArrayStock::add, &stock_addr);
            stock_model.state_emitter.connect(DumpingProcess::check_update_state, &dumping_addr);
            Ok((ComponentModel::DumpingProcess(dumping, dumping_mbox, dumping_addr),
            ComponentModel::ArrayStock(stock_model, stock_mbox, stock_addr)))
        },
        (ComponentModel::DumpingProcess(mut dumping, dumping_mbox, dumping_addr), ComponentModel::TruckStock(mut stock_model, stock_mbox, stock_addr)) => {
            dumping.req_downstreams.1.connect(TruckStock::get_state, &stock_addr);
            dumping.push_downstreams.1.connect(TruckStock::add, &stock_addr);
            stock_model.state_emitter.connect(DumpingProcess::check_update_state, &dumping_addr);
            Ok((ComponentModel::DumpingProcess(dumping, dumping_mbox, dumping_addr),
            ComponentModel::TruckStock(stock_model, stock_mbox, stock_addr)))
        },
        // },
        // (ComponentModel::ArrayStock(_, _, stock_addr), ComponentModel::LoadingProcess(mut loading, _, _)) => {
        //     loading.req_upstreams.0.connect(ArrayStock::get_state, &stock_addr);
        //     loading.withdraw_upstreams.0.connect(ArrayStock::remove, &stock_addr);
        //     Ok(())
        // },
        // (ComponentModel::LoadingProcess(mut loading, _, _), ComponentModel::TruckStock(_, _, stock_addr)) => {
        //     loading.req_downstream.connect(TruckStock::get_state, &stock_addr);
        //     loading.push_downstream.connect(TruckStock::add, &stock_addr);
        //     Ok(())
        // },
        // (ComponentModel::TruckStock(_, _, stock_addr), ComponentModel::TruckMovementProcess(mut movement, _, _)) => {
        //     movement.req_upstream.connect(TruckStock::get_state, &stock_addr);
        //     movement.withdraw_upstream.connect(TruckStock::remove, &stock_addr);
        //     Ok(())
        // },
        // (ComponentModel::TruckMovementProcess(mut movement, _, _), ComponentModel::TruckStock(_, _, stock_addr)) => {
        //     movement.req_downstream.connect(TruckStock::get_state, &stock_addr);
        //     movement.push_downstream.connect(TruckStock::add, &stock_addr);
        //     Ok(())
        // },
        // (ComponentModel::TruckStock(_, _, stock_addr), ComponentModel::DumpingProcess(mut dumping, _, _)) => {
        //     dumping.req_upstream.connect(TruckStock::get_state, &stock_addr);
        //     dumping.withdraw_upstream.connect(TruckStock::remove_any, &stock_addr);
        //     Ok(())
        // },
        // (ComponentModel::DumpingProcess(mut dumping, _, _), ComponentModel::ArrayStock(_, _, stock_addr)) => {
        //     dumping.req_downstreams.0.connect(ArrayStock::get_state, &stock_addr);
        //     dumping.push_downstreams.0.connect(ArrayStock::add, &stock_addr);
        //     Ok(())
        // },
        // (ComponentModel::DumpingProcess(mut dumping, _, _), ComponentModel::TruckStock(_, _, stock_addr)) => {
        //     dumping.req_downstreams.1.connect(TruckStock::get_state, &stock_addr);
        //     dumping.push_downstreams.1.connect(TruckStock::add, &stock_addr);
        //     Ok(())
        // },
        _ => Err(format!("Connection error: Implementation does not exist for instances {} to {}", comp1_name, comp2_name).into()),
    }
}

#[derive(Debug, Clone, Serialize)]
enum LogRecordType {
    TruckingProcessLog(TruckingProcessLog),
    QueueStockLog(QueueStockLog),
    ArrayStockLog(ArrayStockLog),
}

enum EventLogger {
    TruckingProcessLogger(TruckingProcessLogger),
    QueueStockLogger(QueueStockLogger),
    ArrayStockLogger(ArrayStockLogger),
}

impl EventLogger {
    fn get_name(&self) -> &String {
        match self {
            EventLogger::TruckingProcessLogger(x) => x.get_name(),
            EventLogger::QueueStockLogger(x) => x.get_name(),
            EventLogger::ArrayStockLogger(x) => x.get_name(),
        }
    }
}

struct TruckingProcessLogger {
    name: String,
    buffer: EventBuffer<<Self as Logger>::RecordType>,
}

impl Logger for TruckingProcessLogger {

    type RecordType = TruckingProcessLog;
    fn get_name(&self) -> &String {
        &self.name
    }
    fn get_buffer(&self) -> &EventBuffer<Self::RecordType> {
        &self.buffer
    }
    fn write_csv(self, dir: String) -> Result<(), Box<dyn Error>> {
        // let file = File::create(path)?;
        let file = File::create(format!("{}/{}.csv", dir, self.name))?;
        let mut writer = WriterBuilder::new()
            .has_headers(true)
            .from_writer(file);
        self.buffer.for_each(|log| {
            writer.serialize(log).expect("Failed to write log record to CSV file");
        });
        writer.flush()?;
        Ok(())
    }
}

impl TruckingProcessLogger {
    fn new(name: String, buffer_size: usize) -> Self {
        TruckingProcessLogger {
            name,
            buffer: EventBuffer::with_capacity(buffer_size),
        }
    }
}

struct QueueStockLogger {
    name: String,
    buffer: EventBuffer<QueueStockLog>,
}

impl Logger for QueueStockLogger {
    type RecordType = QueueStockLog;
    fn get_name(&self) -> &String {
        &self.name
    }
    fn get_buffer(&self) -> &EventBuffer<Self::RecordType> {
        &self.buffer
    }
    fn write_csv(self, dir: String) -> Result<(), Box<dyn Error>> {
        let file = File::create(format!("{}/{}.csv", dir, self.name))?;
        let mut writer = WriterBuilder::new()
            .has_headers(true)
            .from_writer(file);
        self.buffer.for_each(|log| {
            writer.serialize(log).expect("Failed to write log record to CSV file");
        });
        writer.flush()?;
        Ok(())
    }
}

impl QueueStockLogger {
    fn new(name: String, buffer_size: usize) -> Self {
        QueueStockLogger {
            name,
            buffer: EventBuffer::with_capacity(buffer_size),
        }
    }
}

struct ArrayStockLogger {
    name: String,
    buffer: EventBuffer<ArrayStockLog>,
}

impl Logger for ArrayStockLogger {
    type RecordType = ArrayStockLog;
    fn get_name(&self) -> &String {
        &self.name
    }
    fn get_buffer(&self) -> &EventBuffer<Self::RecordType> {
        &self.buffer
    }
    fn write_csv(self, dir: String) -> Result<(), Box<dyn Error>> {
        // TODO: turn this into a derive macro
        let file = File::create(format!("{}/{}.csv", dir, self.name))?;
        let mut writer = WriterBuilder::new()
            .has_headers(true)
            .from_writer(file);
        self.buffer.for_each(|log| {
            writer.serialize(log).expect("Failed to write log record to CSV file");
        });
        writer.flush()?;
        Ok(())
    }
}

impl ArrayStockLogger {
    fn new(name: String, buffer_size: usize) -> Self {
        ArrayStockLogger {
            name,
            buffer: EventBuffer::with_capacity(buffer_size),
        }
    }
}

trait Logger {
    type RecordType: Serialize;
    fn get_name(&self) -> &String;
    fn get_buffer(&self) -> &EventBuffer<Self::RecordType>;
    fn write_csv(self, dir: String) -> Result<(), Box<dyn Error>>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LoggerConfig {
    name: String,
    record_type: String,
    max_length: usize,
    log_path: String,
}

fn create_logger(config: LoggerConfig) -> Result<EventLogger, Box<dyn Error>> {
    let (name, log_type, max_length) = (config.name, config.record_type, config.max_length);
    match log_type.as_str() {
        "TruckingProcessLog" | "TruckAndOreStockLog" => {
            let buffer = TruckingProcessLogger::new(name, max_length);
            Ok(EventLogger::TruckingProcessLogger(buffer))
        },
        "QueueStockLog" => {
            let buffer = QueueStockLogger::new(name, max_length);
            Ok(EventLogger::QueueStockLogger(buffer))
        },
        "ArrayStockLog" => {
            let buffer = ArrayStockLogger::new(name, max_length);
            Ok(EventLogger::ArrayStockLogger(buffer))
        },
        _ => Err(format!("Unknown log type: {}", log_type).into()),
    }
}

fn connect_logger(component: &mut ComponentModel, logger: &mut EventLogger) -> Result<(), Box<dyn Error>> {
    let component_name = component.get_name().clone();
    let logger_name = logger.get_name().clone();
    match (component, logger) {
        (ComponentModel::ArrayStock(stock, _, _), EventLogger::ArrayStockLogger(logger)) => {
            let x = logger.get_buffer();
            stock.log_emitter.connect_sink(logger.get_buffer());
            Ok(())
        },
        (ComponentModel::TruckStock(stock, _, _), EventLogger::QueueStockLogger(logger)) => {
            let x = logger.get_buffer();
            stock.log_emitter.connect_sink(logger.get_buffer());
            Ok(())
        },
        (ComponentModel::LoadingProcess(loading, _, _), EventLogger::TruckingProcessLogger(logger)) => {
            let x = logger.get_buffer();
            loading.log_emitter.connect_sink(logger.get_buffer());
            Ok(())
        },
        (ComponentModel::DumpingProcess(dumping, _, _), EventLogger::TruckingProcessLogger(logger)) => {
            let x = logger.get_buffer();
            dumping.log_emitter.connect_sink(logger.get_buffer());
            Ok(())
        },
        (ComponentModel::TruckMovementProcess(movement, _, _), EventLogger::TruckingProcessLogger(logger)) => {
            let x = logger.get_buffer();
            movement.log_emitter.connect_sink(logger.get_buffer());
            Ok(())
        },
        _ => Err(format!("Logger connection error: {} to {}", component_name, logger_name).into()),
    }

}

fn build_and_run_model(args: ParsedArgs, config: ModelConfig) {

    let base_seed = args.seed;

    let mut df: DistributionFactory = DistributionFactory {
        base_seed,
        next_seed: base_seed,
    };

    // let stockpile_logger = EventLogger::<ArrayStockLog>::new(100_000);
    // let truck_queue_logger = EventLogger::<QueueStockLog>::new(100_000);
    // // let queue_logger = EventLogger::<QueueStockLog>::new(100_000);
    // let process_logger = EventLogger::<TruckingProcessLog>::new(100_000);

    // let truck_stock_logger = EventLogger::<TruckAndOreStockLog>::new(100_000);

    // let logger_configs: Vec<LoggerConfig> = vec![];
    let mut loggers: IndexMap<String, EventLogger> = IndexMap::new();
    for config in config.loggers {
        // );logger = create_logger(config).unwrap_or_else(|e| {
        //     eprintln!("Error creating logger: {}", e);
        //     EventLogger::TruckingProcessLogger(TruckingProcessLogger::new("default".into(), 100_000))
        // }
        let logger_result: Result<EventLogger, Box<dyn Error>> = create_logger(config);
        match logger_result {
            Ok(logger) => {
                loggers.insert(logger.get_name().clone(), logger);
            }
            Err(e) => {
                eprintln!("Error creating logger: {}", e);
            }
        }
    }

    // let component_configs: Vec<ComponentConfig> = vec![];
    let mut components: IndexMap<String, ComponentModel> = IndexMap::new();
    for config in config.components {
        match config.create_component(&mut df, &mut loggers) {
            Ok(component) => {
                components.insert(component.get_name().clone(), component);
            }
            Err(e) => {
                eprintln!("Error creating component: {}", e);
            }
        }
    }

    // let connections_configs: Vec<ConnectionConfig> = vec![];
    let mut connection_errors: Vec<String> = vec![];

    for connection in config.connections {
        let comp_us = components.swap_remove(&connection.upstream);
        let comp_ds = components.swap_remove(&connection.downstream);
        match (comp_us, comp_ds) {
            (Some(comp1), Some(comp2)) => {
                // println!("Connecting {} to {}", comp1.get_name(), comp2.get_name());
                match connect_components(comp1, comp2) {
                    Ok((comp1, comp2)) => {
                        components.insert(connection.upstream.clone(), comp1);
                        components.insert(connection.downstream.clone(), comp2);
                    }
                    Err(e) => {
                        connection_errors.push(e.to_string());
                    }
                }
            }
            (Some(_), None) => {
                connection_errors.push(format!("Connection error: Component instance {} not defined", connection.downstream));
            },
            (None, Some(_)) => {
                connection_errors.push(format!("Connection error: Component instance {} not defined", connection.upstream));
            },
            (None, None) => {
                connection_errors.push(format!("Connection error: Component instances {} and {} not defined", connection.upstream, connection.downstream));
            }
        }
    }

    if !connection_errors.is_empty() {
        for error in connection_errors {
            eprintln!("{}", error);
            println!("{}", error);
        }
    }

    


    // source_stockpile.resource.add(ArrayResource {
    //     vec: [800000., 600000., 400000., 200000., 0.],
    // });
    // let source_stockpile_mbox: Mailbox<ArrayStock> = Mailbox::new();
    // let source_stockpile_addr = source_stockpile_mbox.address();

    // let mut ready_to_load_trucks = TruckStock::new()
    //     .with_name("ReadyToLoadTrucks".into())
    //     .with_log_consumer(&truck_queue_logger);
    // let truck_stock_mbox: Mailbox<TruckStock> = Mailbox::new();
    // let truck_stock_addr = truck_stock_mbox.address();

    // (0..args.num_trucks).for_each(|i| {
    //     ready_to_load_trucks.resource.add(Some(TruckAndOre {
    //         truck: (100 + i) as i32,
    //         ore: ArrayResource { vec: [0.; 5] },
    //     }));
    // });

    // let mut loading_process = LoadingProcess::default()
    //     .with_name("LoadingProcess".into())
    //     .with_log_consumer(&process_logger);
    // loading_process
    //     .truck_stock_emitter
    //     .connect_sink(&truck_stock_logger.buffer);
    // loading_process.load_time_dist_secs = Some(
    //     df.create(DistributionConfig::TruncNormal {
    //         mean: 40.,
    //         std: 10.,
    //         min: Some(10.),
    //         max: Some(70.),
    //     })
    //     .unwrap(),
    // );
    // loading_process.load_quantity_dist = Some(
    //     df.create(DistributionConfig::Uniform {
    //         min: 70.,
    //         max: 130.,
    //     })
    //     .unwrap(),
    // );
    // let loading_mbox: Mailbox<LoadingProcess> = Mailbox::new();
    // let loading_addr = loading_mbox.address();

    // let mut loaded_trucks = TruckStock::new()
    //     .with_name("LoadedTrucks".into())
    //     .with_log_consumer(&truck_queue_logger);
    // let loaded_trucks_mbox: Mailbox<TruckStock> = Mailbox::new();
    // let loaded_trucks_addr = loaded_trucks_mbox.address();

    // let mut loaded_truck_movement_process = TruckMovementProcess::default()
    //     .with_name("LoadedTruckMovementProcess".into())
    //     .with_log_consumer(&process_logger)
    //     .with_travel_time_dist_secs(Some(
    //         df.create(DistributionConfig::Triangular {
    //             min: 100.,
    //             max: 200.,
    //             mode: 120.,
    //         })
    //         .unwrap(),
    //     ));
    // let loaded_truck_movement_mbox: Mailbox<TruckMovementProcess> = Mailbox::new();
    // let loaded_truck_movement_addr = loaded_truck_movement_mbox.address();

    // let mut ready_to_dump_trucks = TruckStock::new()
    //     .with_name("ReadyToDumpTrucks".into())
    //     .with_log_consumer(&truck_queue_logger);
    // let ready_to_dump_trucks_mbox: Mailbox<TruckStock> = Mailbox::new();
    // let ready_to_dump_trucks_addr = ready_to_dump_trucks_mbox.address();

    // let mut dumping_process = DumpingProcess::default()
    //     .with_name("DumpingProcess".into())
    //     .with_log_consumer(&process_logger);
    // dumping_process
    //     .truck_stock_emitter
    //     .connect_sink(&truck_stock_logger.buffer);
    // dumping_process.dump_time_dist_secs = Some(
    //     df.create(DistributionConfig::TruncNormal {
    //         mean: 30.,
    //         std: 8.,
    //         min: Some(14.),
    //         max: Some(46.),
    //     })
    //     .unwrap(),
    // );
    // let dumping_mbox: Mailbox<DumpingProcess> = Mailbox::new();
    // let dumping_addr = dumping_mbox.address();

    // let mut dumped_stockpile = ArrayStock::new()
    //     .with_name("DumpedStockpile".into())
    //     .with_log_consumer(&stockpile_logger);
    // dumped_stockpile.max_capacity = 999_999_999.;
    // let dumped_stockpile_mbox: Mailbox<ArrayStock> = Mailbox::new();
    // let dumped_stockpile_addr = dumped_stockpile_mbox.address();

    // let mut empty_trucks = TruckStock::new()
    //     .with_name("EmptyTrucks".into())
    //     .with_log_consumer(&truck_queue_logger);
    // let empty_trucks_mbox: Mailbox<TruckStock> = Mailbox::new();
    // let empty_trucks_addr = empty_trucks_mbox.address();

    // let mut empty_truck_movement_process = TruckMovementProcess::default()
    //     .with_name("EmptyTruckMovementProcess".into())
    //     .with_log_consumer(&process_logger);
    // empty_truck_movement_process.travel_time_dist_secs = Some(
    //     df.create(DistributionConfig::Triangular {
    //         min: 50.,
    //         max: 100.,
    //         mode: 60.,
    //     })
    //     .unwrap(),
    // );
    // let empty_truck_movement_mbox: Mailbox<TruckMovementProcess> = Mailbox::new();
    // let empty_truck_movement_addr = empty_truck_movement_mbox.address();

    // loading_process.req_upstreams.0.connect(ArrayStock::get_state, &source_stockpile_addr);
    // loading_process.req_upstreams.1.connect(TruckStock::get_state, &truck_stock_addr);
    // loading_process.withdraw_upstreams.0.connect(ArrayStock::remove, &source_stockpile_addr);
    // loading_process.withdraw_upstreams.1.connect(TruckStock::remove_any, &truck_stock_addr);
    // loading_process.push_downstream.connect(TruckStock::add, &loaded_trucks_addr);
    // source_stockpile.state_emitter.connect(LoadingProcess::check_update_state, &loading_addr);
    // ready_to_load_trucks.state_emitter.connect(LoadingProcess::check_update_state, &loading_addr);

    // loaded_truck_movement_process.req_upstream.connect(TruckStock::get_state, &loaded_trucks_addr);
    // loaded_truck_movement_process.withdraw_upstream.connect(TruckStock::remove, &loaded_trucks_addr);
    // loaded_trucks.state_emitter.connect(TruckMovementProcess::check_update_state,&loaded_truck_movement_addr);
    // loaded_truck_movement_process.req_downstream.connect(TruckStock::get_state, &ready_to_dump_trucks_addr);
    // loaded_truck_movement_process.push_downstream.connect(TruckStock::add, &ready_to_dump_trucks_addr);

    // dumping_process.req_upstream.connect(TruckStock::get_state, &ready_to_dump_trucks_addr);
    // dumping_process.withdraw_upstream.connect(TruckStock::remove_any, &ready_to_dump_trucks_addr);
    // dumping_process.req_downstreams.0.connect(ArrayStock::get_state, &dumped_stockpile_addr);
    // dumping_process.push_downstreams.0.connect(ArrayStock::add, &dumped_stockpile_addr);
    // dumping_process.push_downstreams.1.connect(TruckStock::add, &empty_trucks_addr);
    // ready_to_dump_trucks.state_emitter.connect(DumpingProcess::check_update_state, &dumping_addr);
    // dumped_stockpile.state_emitter.connect(DumpingProcess::check_update_state, &dumping_addr);

    // empty_truck_movement_process.req_upstream.connect(TruckStock::get_state, &empty_trucks_addr);
    // empty_truck_movement_process.withdraw_upstream.connect(TruckStock::remove, &empty_trucks_addr);
    // empty_truck_movement_process.req_downstream.connect(TruckStock::get_state, &truck_stock_addr);
    // empty_truck_movement_process.push_downstream.connect(TruckStock::add, &truck_stock_addr);
    // empty_trucks.state_emitter.connect(TruckMovementProcess::check_update_state, &empty_truck_movement_addr);


    println!("Init loc: {:?}", config.truck_init_location);
    println!("Components: {:?}", components.keys());

    match components.get_mut(&config.truck_init_location) {
        Some(ComponentModel::TruckStock(comp, _, _)) => {
            (0..args.num_trucks).for_each(|i| {
                comp.resource.add(Some(TruckAndOre {
                    truck: (100 + i) as i32,
                    ore: ArrayResource { vec: [0.; 5] },
                }));
            });
        },
        _ => {
            eprintln!("Truck init component not found");
            return;
        }
    };

    let mut sim_init = SimInit::new();
    let mut addresses: IndexMap<String, ComponentModelAddress> = IndexMap::new();

    for (_, component) in components.drain(..) {
        match component {
            ComponentModel::ArrayStock(stock, mbox, addr) => {
                let element_name = stock.element_name.clone();
                addresses.insert(stock.element_name.clone(), ComponentModelAddress::ArrayStock(addr));
                sim_init = sim_init.add_model(stock, mbox, element_name);
            },
            ComponentModel::TruckStock(stock, mbox, addr) => {
                let element_name = stock.element_name.clone();
                addresses.insert(stock.element_name.clone(), ComponentModelAddress::TruckStock(addr));
                sim_init = sim_init.add_model(stock, mbox, element_name);
            },
            ComponentModel::LoadingProcess(loading, mbox, addr) => {
                let element_name = loading.element_name.clone();
                addresses.insert(loading.element_name.clone(), ComponentModelAddress::LoadingProcess(addr));
                sim_init = sim_init.add_model(loading, mbox, element_name);
            },
            ComponentModel::DumpingProcess(dumping, mbox, addr) => {
                let element_name = dumping.element_name.clone();
                addresses.insert(dumping.element_name.clone(), ComponentModelAddress::DumpingProcess(addr));
                sim_init = sim_init.add_model(dumping, mbox, element_name);
            },
            ComponentModel::TruckMovementProcess(movement, mbox, addr) => {
                let element_name = movement.element_name.clone();
                addresses.insert(movement.element_name.clone(), ComponentModelAddress::TruckMovementProcess(addr));
                sim_init = sim_init.add_model(movement, mbox, element_name);
            },
        }
    }


    // let sim_init = SimInit::new()
    //     .add_model(source_stockpile, source_stockpile_mbox, "SourceStockpile")
    //     .add_model(ready_to_load_trucks, truck_stock_mbox, "TruckStock")
    //     .add_model(loading_process, loading_mbox, "LoadingProcess")
    //     .add_model(loaded_trucks, loaded_trucks_mbox, "LoadedTrucks")
    //     .add_model(loaded_truck_movement_process,loaded_truck_movement_mbox,"LoadedTruckMovementProcess")
    //     .add_model(ready_to_dump_trucks,ready_to_dump_trucks_mbox,"ReadyToDumpTrucks")
    //     .add_model(dumping_process, dumping_mbox, "DumpingProcess")
    //     .add_model(dumped_stockpile, dumped_stockpile_mbox, "DumpedStockpile")
    //     .add_model(empty_trucks, empty_trucks_mbox, "EmptyTrucks")
    //     .add_model(empty_truck_movement_process,empty_truck_movement_mbox,"EmptyTruckMovementProcess");

    let start_time = MonotonicTime::try_from_date_time(2025, 1, 1, 0, 0, 0, 0).unwrap();
    let mut simu = sim_init.init(start_time).unwrap().0;

    addresses.iter().for_each(|(name, addr)| {
        let nm = NotificationMetadata {
            time: start_time,
            element_from: name.clone(),
            message: "Start".into(),
        };
        match addr {
            ComponentModelAddress::ArrayStock(addr) =>  {
                simu.process_event(ArrayStock::check_update_state, nm, addr).unwrap();
            },
            ComponentModelAddress::TruckStock(addr) => {
                simu.process_event(TruckStock::check_update_state, nm, addr).unwrap();
            },
            ComponentModelAddress::LoadingProcess(addr) => {
                simu.process_event(LoadingProcess::check_update_state, nm, addr).unwrap();
            },
            ComponentModelAddress::DumpingProcess(addr) => {
                simu.process_event(DumpingProcess::check_update_state, nm, addr).unwrap();
            },
            ComponentModelAddress::TruckMovementProcess(addr) => {
                simu.process_event(TruckMovementProcess::check_update_state, nm, addr).unwrap();
            },
        }
    });

    // simu.process_event(
    //     LoadingProcess::check_update_state,
    //     NotificationMetadata {
    //         time: start_time,
    //         element_from: "Simulation".into(),
    //         message: "Start".into(),
    //     },
    //     &loading_addr,
    // )
    // .unwrap();
    simu.step_until(start_time + Duration::from_secs_f64(args.sim_duration_secs)).unwrap();

    // Create dir if doesn't exist
    let dir = format!("outputs/trucking/{:04}", base_seed);
    if !std::path::Path::new(&dir).exists() {
        std::fs::create_dir_all(&dir).unwrap();
    }

    // loggers.iter_mut
    
    for logger in loggers.drain(..) {
        match logger {
            (_, EventLogger::TruckingProcessLogger(logger)) => {
                let logger_name = logger.get_name().clone();
                logger.write_csv(dir.clone()).unwrap_or_else(|e| {
                    eprintln!("Error writing logger {} to CSV: {}", logger_name, e);
                });
            },
            (_, EventLogger::QueueStockLogger(logger)) => {
                let logger_name = logger.get_name().clone();
                logger.write_csv(dir.clone()).unwrap_or_else(|e| {
                    eprintln!("Error writing logger {} to CSV: {}", logger_name, e);
                });
            },
            (_, EventLogger::ArrayStockLogger(logger)) => {
                let logger_name = logger.get_name().clone();
                logger.write_csv(dir.clone()).unwrap_or_else(|e| {
                    eprintln!("Error writing logger {} to CSV: {}", logger_name, e);
                });
            },
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
struct ModelConfig {
    id: String,
    name: String,
    description: Option<String>,
    truck_init_location: String,
    loggers: Vec<LoggerConfig>,
    components: Vec<ComponentConfig>,
    connections: Vec<ConnectionConfig>,
}

fn main() {
    let args = Args::parse();

    let seeds = match parse_seed_range(&args.seed) {
        Ok(seeds) => seeds,
        Err(err) => {
            eprintln!("Error parsing seed range: {}", err);
            std::process::exit(1);
        }
    };

    let file = File::open("quokkasim/src/my_config_2.yaml").unwrap();
    let reader = BufReader::new(file);
    let config: ModelConfig = serde_yaml::from_reader(reader).unwrap();

    // println!("{:#?}", config);

    seeds.par_iter().for_each(|seed| {
        let args = ParsedArgs {
            seed: *seed,
            num_trucks: args.num_trucks,
            sim_duration_secs: args.sim_duration_secs,
        };
        build_and_run_model(args, config.clone());
    });

}
