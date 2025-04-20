#![allow(unused)]

use std::time::Duration;

use indexmap::{IndexMap, IndexSet};
use nexosim::{
    model::{Context, Model},
    ports::{Output, Requestor},
    time::MonotonicTime,
};
use quokkasim::{
    common::EventLogger, components::{
        array::{ArrayProcessLog, ArrayResource, ArrayStock, ArrayStockLog, ArrayStockState},
        queue::QueueState,
    }, core::{Mailbox, ResourceAdd, ResourceRemove, SimInit, StateEq}, define_combiner_process, define_process, define_splitter_process, define_stock, prelude::{MyQueueStock, QueueStockLog}
};
use serde::{ser::SerializeStruct, Serialize};

#[derive(Debug, Clone)]
pub struct TruckAndOre {
    truck: i32,
    ore: ArrayResource,
}

pub struct TruckAndOreMap {
    trucks: IndexMap<i32, TruckAndOre>,
}

impl ResourceAdd<Vec<TruckAndOre>> for TruckAndOreMap {
    fn add(&mut self, mut truck_and_ore: Vec<TruckAndOre>) {
        while !truck_and_ore.is_empty() {
            let item = truck_and_ore.pop().unwrap();
            self.trucks.insert(item.truck, item);
        }
    }
}

impl ResourceRemove<Vec<i32>, Vec<TruckAndOre>> for TruckAndOreMap {
    fn sub(&mut self, ids: Vec<i32>) -> Vec<TruckAndOre> {
        let mut results = Vec::new();
        for id in ids {
            if let Some(item) = self.trucks.swap_remove(&id) {
                results.push(item);
            }
        }
        results
    }
}

impl Default for TruckAndOreMap {
    fn default() -> Self {
        TruckAndOreMap { trucks: IndexMap::new() }
    }
}

#[derive(Debug, Clone)]
struct TruckingProcessLog {
    pub time: String,
    pub element_name: String,
    pub element_type: String,
    pub process_data: TruckingProcessLogType,
}

impl Serialize for TruckingProcessLog {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("TruckingProcessLog", 10)?;
        state.serialize_field("time", &self.time)?;
        state.serialize_field("element_name", &self.element_name)?;
        state.serialize_field("element_type", &self.element_type)?;

        let (event_type, truck_id, total, x0, x1, x2, x3, x4, reason): (Option<&'static str>, Option<i32>, Option<f64>, Option<f64>, Option<f64>, Option<f64>, Option<f64>, Option<f64>, Option<&'static str>) = match &self.process_data {
            TruckingProcessLogType::LoadStart { truck_id, tonnes, components, .. } => (Some("LoadSuccess"), Some(*truck_id), Some(*tonnes), Some(components[0]), Some(components[1]), Some(components[2]), Some(components[3]), Some(components[4]), None),
            TruckingProcessLogType::LoadSuccess { truck_id, tonnes, components, .. } => (Some("LoadSuccess"), Some(*truck_id), Some(*tonnes), Some(components[0]), Some(components[1]), Some(components[2]), Some(components[3]), Some(components[4]), None),
            TruckingProcessLogType::LoadFailure { reason } => (Some("LoadFailure"), None, None, None, None, None, None, None, Some(*reason)),
            TruckingProcessLogType::DumpSuccess { truck_id, tonnes, components, .. } => (Some("DumpSuccess"), Some(*truck_id), Some(*tonnes), Some(components[0]), Some(components[1]), Some(components[2]), Some(components[3]), Some(components[4]), None),
            TruckingProcessLogType::DumpFailure { reason } => (Some("DumpFailure"), None, None, None, None, None, None, None, Some(*reason)),
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
    LoadStart { truck_id: i32, tonnes: f64, components: [f64; 5] },
    LoadSuccess { truck_id: i32, tonnes: f64, components: [f64; 5] },
    LoadFailure { reason: &'static str },
    DumpSuccess { truck_id: i32, tonnes: f64, components: [f64; 5] },
    DumpFailure { reason: &'static str },
    TruckMovement { truck_id: i32, tonnes: f64, components: [f64; 5] },
}

#[derive(Debug, Clone)]
enum LoaderState {
    Loading { truck: TruckAndOre, previous_check_time: MonotonicTime, time_until_done: Duration },
    Idle,
}

impl Default for LoaderState {
    fn default() -> Self {
        LoaderState::Idle
    }
}

define_combiner_process!(
    /// Loading process for trucks. Draws from a truck stock and array stock, pushes out Vec<TruckAndOre>
    name = LoadingProcess,
    inflow_stock_state_types = (ArrayStockState, TruckStockState),
    resource_in_types = (ArrayResource, Vec<TruckAndOre>),
    resource_in_parameter_types = (f64, ()),
    outflow_stock_state_type = TruckStockState,
    resource_out_type = Vec<TruckAndOre>,
    resource_out_parameter_type = Vec<TruckAndOre>,
    check_update_method = |mut x: Self, time: MonotonicTime| {
        async move {
            // First resolve Loading state, if applicable
            let is_still_loading;
            match x.state.clone() {
                LoaderState::Loading { truck, previous_check_time, time_until_done } => {
                    let elapsed_time = time.duration_since(previous_check_time);
                    let new_time_until_done = time_until_done.saturating_sub(elapsed_time);
                    let new_previous_check_time = time;

                    if time_until_done.is_zero() {
                        x.log(time, TruckingProcessLogType::LoadSuccess { truck_id: truck.truck,  tonnes: truck.ore.total(), components: truck.ore.vec } ).await;
                        x.log_truck_stock(time, TruckAndOreStockLogDetails::StockAdded { total: truck.ore.total(), empty: 999., contents: truck.ore.vec }).await;
                        x.push_downstream.send((vec![truck.clone()], NotificationMetadata {
                            time,
                            element_from: x.element_name.clone(),
                            message: "Truck and ore".into(),
                        })).await;
                        x.state = LoaderState::Idle;
                        is_still_loading = false;
                    } else {
                        x.state = LoaderState::Loading { truck, previous_check_time: new_previous_check_time, time_until_done: new_time_until_done };
                        x.time_to_next_event_counter = Some(time_until_done);
                        is_still_loading = true;
                        return x;
                    }
                },
                LoaderState::Idle => {}
            }

            // Then execute new load
            let us_material_state: ArrayStockState = x.req_upstreams.0.send(()).await.next().unwrap();
            let us_truck_state: TruckStockState = x.req_upstreams.1.send(()).await.next().unwrap();
            println!("{:?} LoadingProcess.check_update_state | upstream_mat {:?} | upstream_truck {:?}", time.to_string(), &us_material_state, &us_truck_state);

            match (&us_material_state, &us_truck_state) {
                (ArrayStockState::Normal { .. } | ArrayStockState::Full { .. }, TruckStockState::Normal { .. }) => {
                    let mut truck = x.withdraw_upstreams.1.send(((), NotificationMetadata {
                        time,
                        element_from: x.element_name.clone(),
                        message: "Truck request".into(),
                    })).await.next().unwrap();
                    let material = x.withdraw_upstreams.0.send((100., NotificationMetadata {
                        time,
                        element_from: x.element_name.clone(),
                        message: "Material request".into(),
                    })).await.next().unwrap();
                    let truck_id = truck.get(0).unwrap().truck;
                    truck.get_mut(0).unwrap().ore = material.clone();
                    println!("{:?} LoadingProcess.push_downstream.send({:?})", time.to_string(), truck.clone());
                    // x.push_downstream.send((truck, NotificationMetadata {
                    //     time,
                    //     element_from: x.element_name.clone(),
                    //     message: "Truck and ore".into(),
                    // })).await;
                    x.state = LoaderState::Loading { truck: truck.swap_remove(0), previous_check_time: time.clone(), time_until_done: Duration::from_secs(15) };
                    x.log(time, TruckingProcessLogType::LoadSuccess { truck_id,  tonnes: material.total(), components: material.vec.clone() } ).await;
                    // x.log_truck_stock(time, TruckAndOreStockLogDetails::StockAdded { total: material.total(), empty: 999., contents: material.vec }).await;
                    x.time_to_next_event_counter = Some(Duration::from_secs(10));
                },
                (ArrayStockState::Empty { .. }, _) => {
                    x.log(time, TruckingProcessLogType::LoadFailure { reason: "No material available" }).await;
                    // x.time_to_next_event_counter = Duration::from_secs(10);
                    x.time_to_next_event_counter = None;
                },
                (_, TruckStockState::Empty) => {
                    x.log(time, TruckingProcessLogType::LoadFailure { reason: "No trucks available" }).await;
                    // x.time_to_next_event_counter = Duration::from_secs(10);
                    x.time_to_next_event_counter = None;
                }
            }
            x
        }
    },
    fields = {
        state: LoaderState,
        truck_stock_emitter: Output<TruckAndOreStockLog>
    },
    log_record_type = TruckingProcessLog,
    log_method = |x: &'a mut Self, time: MonotonicTime, details: TruckingProcessLogType| {
        async move {
            let log = TruckingProcessLog {
                time: time.to_chrono_date_time(0).unwrap().to_string(),
                element_name: x.element_name.clone(),
                element_type: x.element_type.clone(),
                process_data: details,
            };
            x.log_emitter.send(log).await;
        }
    },
    log_method_parameter_type = TruckingProcessLogType
);

impl LoadingProcess {
    pub fn log_truck_stock(&mut self, time: MonotonicTime, details: TruckAndOreStockLogDetails) -> impl Future<Output=()> {
        async move {
            let (log_type, occupied, empty, contents) = match details {
                TruckAndOreStockLogDetails::StockAdded { total, empty, contents } => {
                    ("StockAdded".into(), total, empty, contents)
                },
                TruckAndOreStockLogDetails::StockRemoved { total, empty, contents } => {
                    ("StockRemoved".into(), total, empty, contents)
                }
            };
            let log: TruckAndOreStockLog = TruckAndOreStockLog {
                time: time.to_string(),
                element_name: self.element_name.clone(),
                element_type: self.element_type.clone(),
                log_type, occupied, empty, contents
            };
            println!("log_truck_stock: {:?}", log.clone());
            self.truck_stock_emitter.send(log);
        }
    }
}

define_process!(
    /// TruckMovementProcess
    name = TruckMovementProcess,
    stock_state_type = TruckStockState,
    resource_in_type = Vec<TruckAndOre>,
    resource_in_parameter_type = Vec<i32>,
    resource_out_type = Vec<TruckAndOre>,
    resource_out_parameter_type = Vec<TruckAndOre>,
    check_update_method = |mut x: Self, time: MonotonicTime| {
        async move {
            println!("TruckMovementProcess check_update_method: {:?}", time.to_string());
            let elapsed_time: Duration = match x.previous_check_time {
                None => Duration::MAX,
                Some(t) => time.duration_since(t),
            };

            let mut items_ready: Vec<i32> = vec![];
            x.time_to_next_event_counter = Duration::MAX; 
            for (id, counter) in x.time_counters.iter_mut() {
                *counter = counter.saturating_sub(elapsed_time);
                if counter.is_zero() {
                    items_ready.push(*id);
                } else {
                    x.time_to_next_event_counter = x.time_to_next_event_counter.min(*counter);
                }
            }
            
            // Check for trucks that are done
            for id in items_ready {
                x.time_counters.swap_remove(&id);
                let truck_and_ores: Vec<TruckAndOre> = x.withdraw_upstream.send((vec![id], NotificationMetadata {
                    time,
                    element_from: x.element_name.clone(),
                    message: "Truck request".into(),
                })).await.next().unwrap();
                x.push_downstream.send((truck_and_ores.clone(), NotificationMetadata {
                    time,
                    element_from: x.element_name.clone(),
                    message: "Truck and ore".into(),
                })).await;
                x.log(time, TruckingProcessLogType::TruckMovement { truck_id: id, tonnes: truck_and_ores.first().unwrap().ore.total(), components: truck_and_ores.first().unwrap().ore.vec } ).await;
            }

            // Check for new trucks upstream. If new, add a counter for it
            let us_state: TruckStockState = x.req_upstream.send(()).await.next().unwrap();
            println!("TruckMovementProcess us_state: {:?}", us_state);

            match us_state {
                TruckStockState::Normal(y) => {
                    for id in y.iter() {
                        if !x.time_counters.contains_key(id) {
                            x.time_counters.insert(*id, Duration::from_secs(120));
                            x.time_to_next_event_counter = x.time_to_next_event_counter.min(Duration::from_secs(120));
                        }
                    }
                },
                TruckStockState::Empty => {}
            }
            x
        }
    },
    fields = {
        time_counters: IndexMap<i32, Duration>
    },
    log_record_type = TruckingProcessLog,
    log_method = |x: &'a mut Self, time: MonotonicTime, details: TruckingProcessLogType| {
        async move {
            let log = TruckingProcessLog {
                time: time.to_chrono_date_time(0).unwrap().to_string(),
                element_name: x.element_name.clone(),
                element_type: x.element_type.clone(),
                process_data: details,
            };
            x.log_emitter.send(log).await;
        }
    },
    log_method_parameter_type = TruckingProcessLogType
);

define_splitter_process!(
    /// DumpingProcess
    name = DumpingProcess,
    inflow_stock_state_type = TruckStockState,
    resource_in_type = Vec<TruckAndOre>,
    resource_in_parameter_type = (),
    outflow_stock_state_types = (ArrayStockState, TruckStockState),
    resource_out_types = (ArrayResource, Vec<TruckAndOre>),
    resource_out_parameter_types = (ArrayResource, Vec<TruckAndOre>),
    check_update_method = |mut x: Self, time: MonotonicTime| {
        async move {
            let us_state: TruckStockState = x.req_upstream.send(()).await.next().unwrap();
            let ds_material_state: ArrayStockState = x.req_downstreams.0.send(()).await.next().unwrap();
            match (us_state, ds_material_state) {
                (TruckStockState::Normal { .. }, ArrayStockState::Normal { .. } | ArrayStockState::Empty { .. }) => {
                    let mut truck_and_ore: Vec<TruckAndOre> = x.withdraw_upstream.send(((), NotificationMetadata {
                        time,
                        element_from: x.element_name.clone(),
                        message: "Truck request".into(),
                    })).await.next().unwrap();

                    let mut indexes_to_push: Vec<usize> = vec![];
                    for (index, truck) in truck_and_ore.iter_mut().enumerate() {
                        let material = truck.ore.clone();
                        truck.ore.vec = [0.; 5];
                        x.push_downstreams.0.send((material.clone(), NotificationMetadata {
                            time,
                            element_from: x.element_name.clone(),
                            message: "Material request".into(),
                        })).await;
                        indexes_to_push.push(index);
                        x.log(time, TruckingProcessLogType::DumpSuccess { truck_id: truck.truck, tonnes: material.total(), components: material.vec } ).await;
                        x.time_to_next_event_counter = Duration::from_secs(10);
                    }
                    for index in indexes_to_push.iter().rev() {
                        let truck = truck_and_ore.remove(*index);
                        x.push_downstreams.1.send((vec![truck], NotificationMetadata {
                            time,
                            element_from: x.element_name.clone(),
                            message: "Truck done".into(),
                        })).await;
                    }
                },
                (TruckStockState::Empty, _) => {
                    x.log(time, TruckingProcessLogType::DumpFailure { reason: "No trucks available" }).await;
                    x.time_to_next_event_counter = Duration::from_secs(10);
                },
                (_, ArrayStockState::Full { .. }) => {
                    x.log(time, TruckingProcessLogType::DumpFailure { reason: "Downstream material stock is full" }).await;
                    x.time_to_next_event_counter = Duration::from_secs(10);
                },
            }
            x
        }
    },
    fields = {
        truck_stock_emitter: Output<TruckAndOreStockLog>
    },
    log_record_type = TruckingProcessLog,
    log_method = |x: &'a mut Self, time: MonotonicTime, details: TruckingProcessLogType| {
        async move {
            let log = TruckingProcessLog {
                time: time.to_chrono_date_time(0).unwrap().to_string(),
                element_name: x.element_name.clone(),
                element_type: x.element_type.clone(),
                process_data: details,
            };
            x.log_emitter.send(log).await;
        }
    },
    log_method_parameter_type = TruckingProcessLogType
);

impl DumpingProcess {
    pub fn log_truck_stock(mut self, time: MonotonicTime, details: TruckAndOreStockLogDetails) -> impl Future<Output=()> {
        async move {
            let (log_type, occupied, empty, contents) = match details {
                TruckAndOreStockLogDetails::StockAdded { total, empty, contents } => {
                    ("StockAdded".into(), total, empty, contents)
                },
                TruckAndOreStockLogDetails::StockRemoved { total, empty, contents } => {
                    ("StockRemoved".into(), total, empty, contents)
                }
            };
            let log: TruckAndOreStockLog = TruckAndOreStockLog {
                time: time.to_string(),
                element_name: self.element_name,
                element_type: self.element_type,
                log_type, occupied, empty, contents
            };
            self.truck_stock_emitter.send(log);
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

#[derive(Serialize, Debug, Clone)]
pub struct TruckAndOreStockLog {
    pub time: String,
    pub element_name: String,
    pub element_type: String,
    pub log_type: String,
    pub occupied: f64,
    pub empty: f64,
    pub contents: [f64; 5],
}

#[derive(Serialize, Debug, Clone)]
pub enum TruckAndOreStockLogDetails {
    StockAdded { total: f64, empty: f64, contents: [f64; 5] },
    StockRemoved { total: f64, empty: f64, contents: [f64; 5] },
}

define_stock!(
    /// TruckStock
    name = TruckStock,
    resource_type = TruckAndOreMap,
    initial_resource = Default::default(),
    add_type = Vec<TruckAndOre>,
    remove_type = Vec<TruckAndOre>,
    remove_parameter_type = Vec<i32>,
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
            let state = x.get_state().await;
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
    pub fn remove_any(&mut self, data: ((), NotificationMetadata), cx: &mut Context<Self>) -> impl Future<Output=Vec<TruckAndOre>> {
        async move {
            self.prev_state = Some(self.get_state().await);
            let truck_and_ore_vec = match self.resource.trucks.pop() {
                Some((_, x)) => vec![x],
                None => vec![],
            };
            self.log(data.1.time, "SomeLog".into()).await;
            self.check_update_state(data.1, cx);
            truck_and_ore_vec
        }
    }
}

fn main() {
    let stock_logger = EventLogger::<ArrayStockLog>::new(100_000);
    let stock_logger_2 = EventLogger::<QueueStockLog>::new(100_000);
    let queue_logger = EventLogger::<QueueStockLog>::new(100_000);
    let process_logger = EventLogger::<TruckingProcessLog>::new(100_000);

    let truck_stock_logger = EventLogger::<TruckAndOreStockLog>::new(100_000);

    let mut source_stockpile = ArrayStock::new()
        .with_name("SourceStockpile".into())
        .with_log_consumer(&stock_logger);
    source_stockpile.resource.add(ArrayResource { vec: [8000., 6000., 4000., 2000., 0.] });
    let source_stockpile_mbox: Mailbox<ArrayStock> = Mailbox::new();
    let source_stockpile_addr = source_stockpile_mbox.address();

    let mut ready_to_load_trucks = TruckStock::new()
        .with_name("ReadyToLoadTrucks".into())
        .with_log_consumer(&queue_logger);
    let truck_stock_mbox: Mailbox<TruckStock> = Mailbox::new();
    let truck_stock_addr = truck_stock_mbox.address();

    ready_to_load_trucks.resource.add(vec![
        TruckAndOre { truck: 101, ore: ArrayResource { vec: [0.; 5] } },
        // TruckAndOre { truck: 102, ore: ArrayResource { vec: [0.; 5] } },
        // TruckAndOre { truck: 103, ore: ArrayResource { vec: [0.; 5] } },
        // TruckAndOre { truck: 104, ore: ArrayResource { vec: [0.; 5] } },
    ]);

    let mut loading_process = LoadingProcess::default().with_name("LoadingProcess".into())
        .with_log_consumer(&process_logger);
    loading_process.truck_stock_emitter.connect_sink(&truck_stock_logger.buffer);
    let loading_mbox: Mailbox<LoadingProcess> = Mailbox::new();
    let loading_addr = loading_mbox.address();

    let mut loaded_trucks = TruckStock::new()
        .with_name("LoadedTrucks".into())
        .with_log_consumer(&stock_logger_2);
    let loaded_trucks_mbox: Mailbox<TruckStock> = Mailbox::new();
    let loaded_trucks_addr = loaded_trucks_mbox.address();

    let mut loaded_truck_movement_process = TruckMovementProcess::default()
        .with_name("LoadedTruckMovementProcess".into())
        .with_log_consumer(&process_logger);
    let loaded_truck_movement_mbox: Mailbox<TruckMovementProcess> = Mailbox::new();
    let loaded_truck_movement_addr = loaded_truck_movement_mbox.address();

    let mut ready_to_dump_trucks = TruckStock::new()
        .with_name("ReadyToDumpTrucks".into())
        .with_log_consumer(&stock_logger_2);
    let ready_to_dump_trucks_mbox: Mailbox<TruckStock> = Mailbox::new();
    let ready_to_dump_trucks_addr = ready_to_dump_trucks_mbox.address();

    let mut dumping_process = DumpingProcess::default()
        .with_name("DumpingProcess".into())
        .with_log_consumer(&process_logger);
    dumping_process.truck_stock_emitter.connect_sink(&truck_stock_logger.buffer);
    let dumping_mbox: Mailbox<DumpingProcess> = Mailbox::new();
    let dumping_addr = dumping_mbox.address();

    let mut dumped_stockpile = ArrayStock::new()
        .with_name("DumpedStockpile".into())
        .with_log_consumer(&stock_logger);
    dumped_stockpile.max_capacity = 999_999_999.;
    let dumped_stockpile_mbox: Mailbox<ArrayStock> = Mailbox::new();
    let dumped_stockpile_addr = dumped_stockpile_mbox.address();

    let mut empty_trucks = TruckStock::new()
        .with_name("EmptyTrucks".into())
        .with_log_consumer(&queue_logger);
    let empty_trucks_mbox: Mailbox<TruckStock> = Mailbox::new();
    let empty_trucks_addr = empty_trucks_mbox.address();

    let mut empty_truck_movement_process = TruckMovementProcess::default()
        .with_name("EmptyTruckMovementProcess".into())
        .with_log_consumer(&process_logger);
    let empty_truck_movement_mbox: Mailbox<TruckMovementProcess> = Mailbox::new();
    let empty_truck_movement_addr = empty_truck_movement_mbox.address();

    loading_process.req_upstreams.0.connect(ArrayStock::get_state, &source_stockpile_addr);
    loading_process.req_upstreams.1.connect(TruckStock::get_state, &truck_stock_addr);
    loading_process.withdraw_upstreams.0.connect(ArrayStock::remove, &source_stockpile_addr);
    loading_process.withdraw_upstreams.1.connect(TruckStock::remove_any, &truck_stock_addr);
    loading_process.push_downstream.connect(TruckStock::add, &loaded_trucks_addr);
    source_stockpile.state_emitter.connect(LoadingProcess::check_update_state, &loading_addr);
    ready_to_load_trucks.state_emitter.connect(LoadingProcess::check_update_state, &loading_addr);

    loaded_truck_movement_process.req_upstream.connect(TruckStock::get_state, &loaded_trucks_addr);
    loaded_truck_movement_process.withdraw_upstream.connect(TruckStock::remove, &loaded_trucks_addr);
    loaded_trucks.state_emitter.connect(TruckMovementProcess::check_update_state, &loaded_truck_movement_addr);
    loaded_truck_movement_process.req_downstream.connect(TruckStock::get_state, &ready_to_dump_trucks_addr);
    loaded_truck_movement_process.push_downstream.connect(TruckStock::add, &ready_to_dump_trucks_addr);

    dumping_process.req_upstream.connect(TruckStock::get_state, &ready_to_dump_trucks_addr);
    dumping_process.withdraw_upstream.connect(TruckStock::remove_any, &ready_to_dump_trucks_addr);
    dumping_process.req_downstreams.0.connect(ArrayStock::get_state, &dumped_stockpile_addr);
    dumping_process.push_downstreams.0.connect(ArrayStock::add, &dumped_stockpile_addr);
    dumping_process.push_downstreams.1.connect(TruckStock::add, &empty_trucks_addr);
    ready_to_dump_trucks.state_emitter.connect(DumpingProcess::check_update_state, &dumping_addr);
    dumped_stockpile.state_emitter.connect(DumpingProcess::check_update_state, &dumping_addr);
    
    empty_truck_movement_process.req_upstream.connect(TruckStock::get_state, &empty_trucks_addr);
    empty_truck_movement_process.withdraw_upstream.connect(TruckStock::remove, &empty_trucks_addr);
    empty_truck_movement_process.req_downstream.connect(TruckStock::get_state, &truck_stock_addr);
    empty_truck_movement_process.push_downstream.connect(TruckStock::add, &truck_stock_addr);
    empty_trucks.state_emitter.connect(TruckMovementProcess::check_update_state, &empty_truck_movement_addr);

    let sim_init = SimInit::new()
        .add_model(source_stockpile, source_stockpile_mbox, "SourceStockpile")
        .add_model(ready_to_load_trucks, truck_stock_mbox, "TruckStock")
        .add_model(loading_process, loading_mbox, "LoadingProcess")
        .add_model(loaded_trucks, loaded_trucks_mbox, "LoadedTrucks")
        .add_model(loaded_truck_movement_process, loaded_truck_movement_mbox, "LoadedTruckMovementProcess")
        .add_model(ready_to_dump_trucks, ready_to_dump_trucks_mbox, "ReadyToDumpTrucks")
        .add_model(dumping_process, dumping_mbox, "DumpingProcess")
        .add_model(dumped_stockpile, dumped_stockpile_mbox, "DumpedStockpile")
        .add_model(empty_trucks, empty_trucks_mbox, "EmptyTrucks")
        .add_model(empty_truck_movement_process, empty_truck_movement_mbox, "EmptyTruckMovementProcess");

    let start_time = MonotonicTime::try_from_date_time(2025, 1, 1, 0, 0, 0, 0).unwrap();
    let mut simu = sim_init.init(start_time).unwrap().0;
    simu.process_event(
        LoadingProcess::check_update_state,
        NotificationMetadata {
            time: start_time,
            element_from: "Simulation".into(),
            message: "Start".into(),
        },
        &loading_addr,
    );
    simu.step_until(start_time +  Duration::from_secs_f64(3600.)).unwrap();

    process_logger.write_csv("outputs/trucking_process_logs.csv").unwrap();
    truck_stock_logger.write_csv("outputs/trucking_truck_stock_logs.csv").unwrap();
    stock_logger_2.write_csv("outputs/trucking_stock_logs_2.csv").unwrap();
}
