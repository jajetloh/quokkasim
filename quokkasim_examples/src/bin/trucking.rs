#![allow(unused)]

use std::{collections::HashMap, time::Duration};

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
    trucks: HashMap<i32, TruckAndOre>,
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
            if let Some(item) = self.trucks.remove(&id) {
                results.push(item);
            }
        }
        results
    }
}

impl Default for TruckAndOreMap {
    fn default() -> Self {
        TruckAndOreMap { trucks: HashMap::new() }
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

        let (event_type, total, x0, x1, x2, x3, x4, reason): (Option<&'static str>, Option<f64>, Option<f64>, Option<f64>, Option<f64>, Option<f64>, Option<f64>, Option<&'static str>) = match &self.process_data {
            TruckingProcessLogType::LoadSuccess { tonnes, components, .. } => (Some("LoadSuccess"), Some(*tonnes), Some(components[0]), Some(components[1]), Some(components[2]), Some(components[3]), Some(components[4]), None),
            TruckingProcessLogType::LoadFailure { reason } => (Some("LoadFailure"), None, None, None, None, None, None, Some(*reason)),
            TruckingProcessLogType::DumpSuccess { tonnes, components, .. } => (Some("DumpSuccess"), Some(*tonnes), Some(components[0]), Some(components[1]), Some(components[2]), Some(components[3]), Some(components[4]), None),
            TruckingProcessLogType::DumpFailure { reason } => (Some("DumpFailure"), None, None, None, None, None, None, Some(*reason)),
        };

        state.serialize_field("event_type", &event_type)?;
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
    LoadSuccess { tonnes: f64, components: [f64; 5] },
    LoadFailure { reason: &'static str },
    DumpSuccess { tonnes: f64, components: [f64; 5] },
    DumpFailure { reason: &'static str },
}

define_combiner_process!(
    /// Loading process for trucks. Draws from a truck stock and array stock, pushes out Vec<TruckAndOre>
    name = LoadingProcess,
    inflow_stock_state_types = (ArrayStockState, QueueState),
    resource_in_types = (ArrayResource, Vec<i32>),
    resource_in_parameter_types = (f64, i32),
    outflow_stock_state_type = QueueState,
    resource_out_type = Vec<TruckAndOre>,
    resource_out_parameter_type = Vec<TruckAndOre>,
    check_update_method = |mut x: Self, time: MonotonicTime| {
        async move {
            let us_truck_state: ArrayStockState = x.req_upstreams.0.send(()).await.next().unwrap();
            let us_material_state: QueueState = x.req_upstreams.1.send(()).await.next().unwrap();
            match (us_truck_state, us_material_state) {
                (ArrayStockState::Normal { .. } | ArrayStockState::Full { .. }, QueueState::Normal { .. } | QueueState::Full { .. }) => {
                    let truck_id = *x.withdraw_upstreams.1.send((1, NotificationMetadata {
                        time,
                        element_from: x.element_name.clone(),
                        message: "Truck request".into(),
                    })).await.next().unwrap().first().unwrap();
                    let material = x.withdraw_upstreams.0.send((100., NotificationMetadata {
                        time,
                        element_from: x.element_name.clone(),
                        message: "Material request".into(),
                    })).await.next().unwrap();
                    let truck_and_ore = vec![TruckAndOre { truck: truck_id, ore: material.clone() }];
                    x.push_downstream.send((truck_and_ore.clone(), NotificationMetadata {
                        time,
                        element_from: x.element_name.clone(),
                        message: "Truck and ore".into(),
                    })).await;
                    x.log(time, TruckingProcessLogType::LoadSuccess { tonnes: material.total(), components: material.vec } ).await;
                    x.time_to_next_event_counter = Duration::from_secs(10);
                },
                (ArrayStockState::Empty { .. }, _) => {
                    x.log(time, TruckingProcessLogType::LoadFailure { reason: "No material available" }).await;
                    x.time_to_next_event_counter = Duration::from_secs(10);
                },
                (_, QueueState::Empty { .. }) => {
                    x.log(time, TruckingProcessLogType::LoadFailure { reason: "No trucks available" }).await;
                    x.time_to_next_event_counter = Duration::from_secs(10);
                }
            }
            x
        }
    },
    fields = {},
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

define_process!(
    /// LoadedTrucks
    name = LoadedTruckMovementProcess,
    stock_state_type = LoadedHaulStock,
    resource_in_type = Vec<TruckAndOre>,
    resource_in_parameter_type = i32,
    resource_out_type = Vec<TruckAndOre>,
    resource_out_parameter_type = Vec<TruckAndOre>,
    check_update_method = |mut x: Self, time: MonotonicTime| {
        async move {
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
            for id in items_ready {
                x.time_counters.remove(&id);
                let truck_and_ores: Vec<TruckAndOre> = x.withdraw_upstream.send((id, NotificationMetadata {
                    time,
                    element_from: x.element_name.clone(),
                    message: "Truck request".into(),
                })).await.next().unwrap();
                x.push_downstream.send((truck_and_ores.clone(), NotificationMetadata {
                    time,
                    element_from: x.element_name.clone(),
                    message: "Truck and ore".into(),
                })).await;
                x.log(time, TruckingProcessLogType::LoadSuccess { tonnes: truck_and_ores.first().unwrap().ore.total(), components: truck_and_ores.first().unwrap().ore.vec } ).await;
            }
            x
        }
    },
    fields = {
        time_counters: HashMap<i32, Duration>
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
    inflow_stock_state_type = QueueState,
    resource_in_type = Vec<TruckAndOre>,
    resource_in_parameter_type = i32,
    outflow_stock_state_types = (ArrayStockState, QueueState),
    resource_out_types = (ArrayResource, Vec<i32>),
    resource_out_parameter_types = (ArrayResource, Vec<i32>),
    check_update_method = |mut x: Self, time: MonotonicTime| {
        async move {
            let us_state: QueueState = x.req_upstream.send(()).await.next().unwrap();
            let ds_material_state: ArrayStockState = x.req_downstreams.0.send(()).await.next().unwrap();
            let ds_truck_state: QueueState = x.req_downstreams.1.send(()).await.next().unwrap();
            match (us_state, ds_material_state, ds_truck_state) {
                (QueueState::Normal { .. } | QueueState::Full { .. }, ArrayStockState::Normal { .. } | ArrayStockState::Empty { .. }, QueueState::Normal { .. } | QueueState::Empty { .. }) => {
                    let truck_and_ore: Vec<TruckAndOre> = x.withdraw_upstream.send((1, NotificationMetadata {
                        time,
                        element_from: x.element_name.clone(),
                        message: "Truck request".into(),
                    })).await.next().unwrap();

                    for truck in truck_and_ore.iter() {
                        let truck_id = truck.truck;
                        let material = truck.ore.clone();
                        x.push_downstreams.0.send((material.clone(), NotificationMetadata {
                            time,
                            element_from: x.element_name.clone(),
                            message: "Material request".into(),
                        })).await;
                        x.push_downstreams.1.send((vec![truck_id], NotificationMetadata {
                            time,
                            element_from: x.element_name.clone(),
                            message: "Truck request".into(),
                        })).await;
                        x.log(time, TruckingProcessLogType::DumpSuccess { tonnes: material.total(), components: material.vec } ).await;
                        x.time_to_next_event_counter = Duration::from_secs(10);
                    }
                },
                (QueueState::Empty { .. }, _, _) => {
                    x.log(time, TruckingProcessLogType::DumpFailure { reason: "No trucks available" }).await;
                    x.time_to_next_event_counter = Duration::from_secs(10);
                },
                (_, ArrayStockState::Full { .. }, _) => {
                    x.log(time, TruckingProcessLogType::DumpFailure { reason: "Downstream material stock is full" }).await;
                    x.time_to_next_event_counter = Duration::from_secs(10);
                },
                (_, _, QueueState::Full { .. }) => {
                    x.log(time, TruckingProcessLogType::DumpFailure { reason: "Downstream truck queue is full" }).await;
                    x.time_to_next_event_counter = Duration::from_secs(10);
                }
            }
            x
        }
    },
    fields = {},
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

#[derive(Debug, Clone)]
pub struct LoadedHaulStockState {
    occupied: i32,
}

impl StateEq for LoadedHaulStockState {
    fn is_same_state(&self, other: &Self) -> bool {
        self.occupied == other.occupied
    }
}

define_stock!(
    /// LoadedHaulStock
    name = LoadedHaulStock,
    resource_type = TruckAndOreMap,
    initial_resource = Default::default(),
    add_type = Vec<TruckAndOre>,
    remove_type = Vec<TruckAndOre>,
    remove_parameter_type = Vec<i32>,
    state_type = LoadedHaulStockState,
    fields = {
        low_capacity: f64,
        max_capacity: f64
    },
    get_state_method = |x: &Self| -> LoadedHaulStockState {
        LoadedHaulStockState {
            occupied: x.resource.trucks.len() as i32,
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
                empty: -1,
                state: "".into(),
                contents: "".into(),
            };
            x.log_emitter.send(log).await;
        }
    }
);

fn main() {
    let stock_logger = EventLogger::<ArrayStockLog>::new(100_000);
    let stock_logger_2 = EventLogger::<QueueStockLog>::new(100_000);
    let queue_logger = EventLogger::<QueueStockLog>::new(100_000);
    let process_logger = EventLogger::<TruckingProcessLog>::new(100_000);

    let mut source_stockpile = ArrayStock::new()
        .with_name("SourceStockpile".into())
        .with_log_consumer(&stock_logger);
    source_stockpile.resource.add(ArrayResource { vec: [400., 300., 200., 100., 0.] });
    let source_stockpile_mbox: Mailbox<ArrayStock> = Mailbox::new();
    let source_stockpile_addr = source_stockpile_mbox.address();

    let mut ready_to_load_trucks = MyQueueStock::new()
        .with_name("TruckStock".into())
        .with_log_consumer(&queue_logger);
    ready_to_load_trucks.resource.queue.push(100);
    let truck_stock_mbox: Mailbox<MyQueueStock> = Mailbox::new();
    let truck_stock_addr = truck_stock_mbox.address();

    let mut loading_process = LoadingProcess::default().with_name("LoadingProcess".into())
        .with_log_consumer(&process_logger);
    let loading_mbox: Mailbox<LoadingProcess> = Mailbox::new();
    let loading_addr = loading_mbox.address();

    let loaded_trucks = LoadedHaulStock::new()
        .with_name("LoadedTrucks".into())
        .with_log_consumer(&stock_logger_2);
    let loaded_trucks_mbox: Mailbox<LoadedHaulStock> = Mailbox::new();
    let loaded_trucks_addr = loaded_trucks_mbox.address();

    let mut loaded_truck_movement_process = LoadedTruckMovementProcess::default()
        .with_name("LoadedTruckMovementProcess".into())
        .with_log_consumer(&process_logger);
    let loaded_truck_movement_mbox: Mailbox<LoadedTruckMovementProcess> = Mailbox::new();
    let loaded_truck_movement_addr = loaded_truck_movement_mbox.address();

    let mut ready_to_dump_trucks = LoadedHaulStock::new()
        .with_name("ReadyToDumpTrucks".into())
        .with_log_consumer(&stock_logger_2);
    let ready_to_dump_trucks_mbox: Mailbox<LoadedHaulStock> = Mailbox::new();
    let ready_to_dump_trucks_addr = ready_to_dump_trucks_mbox.address();

    let mut dumping_process = DumpingProcess::default()
        .with_name("DumpingProcess".into())
        .with_log_consumer(&process_logger);
    let dumping_mbox: Mailbox<DumpingProcess> = Mailbox::new();
    let dumping_addr = dumping_mbox.address();

    let mut dumped_stockpile = ArrayStock::new()
        .with_name("DumpedStockpile".into())
        .with_log_consumer(&stock_logger);
    let dumped_stockpile_mbox: Mailbox<ArrayStock> = Mailbox::new();
    let dumped_stockpile_addr = dumped_stockpile_mbox.address();

    let mut empty_trucks = MyQueueStock::new()
        .with_name("EmptyTrucks".into())
        .with_log_consumer(&queue_logger);
    let empty_trucks_mbox: Mailbox<MyQueueStock> = Mailbox::new();
    let empty_trucks_addr = empty_trucks_mbox.address();    

    loading_process.req_upstreams.0.connect(ArrayStock::get_state, &source_stockpile_addr);
    loading_process.req_upstreams.1.connect(MyQueueStock::get_state, &truck_stock_addr);
    loading_process.withdraw_upstreams.0.connect(ArrayStock::remove, &source_stockpile_addr);
    loading_process.withdraw_upstreams.1.connect(MyQueueStock::remove, &truck_stock_addr);
    loading_process.push_downstream.connect(LoadedHaulStock::add, &loaded_trucks_addr);

    let sim_init = SimInit::new()
        .add_model(source_stockpile, source_stockpile_mbox, "SourceStockpile")
        .add_model(ready_to_load_trucks, truck_stock_mbox, "TruckStock")
        .add_model(loading_process, loading_mbox, "LoadingProcess")
        .add_model(loaded_trucks, loaded_trucks_mbox, "LoadedTrucks");

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
}
