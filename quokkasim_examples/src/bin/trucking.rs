#![allow(unused)]

use std::time::Duration;

use nexosim::{
    model::{Context, Model},
    ports::{Output, Requestor},
    time::MonotonicTime,
};
use quokkasim::{
    common::EventLogger,
    components::{
        array::{ArrayProcessLog, ArrayResource, ArrayStock, ArrayStockLog, ArrayStockState},
        queue::QueueState,
    },
    core::{Mailbox, ResourceAdd, ResourceRemove, SimInit},
    define_stock, prelude::{MyQueueStock, QueueStockLog},
};

#[derive(Debug, Clone)]
pub struct TruckAndOre {
    truck: i32,
    ore: ArrayResource,
}

pub struct LoadingProcess {
    element_name: String,
    element_type: String,
    req_material_upstream: Requestor<(), ArrayStockState>,
    withdraw_material_upstream: Requestor<(f64, NotificationMetadata), ArrayResource>,
    req_trucks_upstream: Requestor<(), QueueState>,
    withdraw_trucks_upstream: Requestor<(i32, NotificationMetadata), Vec<i32>>,

    // No need to query downstream state
    push_downstream: Output<(Vec<TruckAndOre>, NotificationMetadata)>,
}

impl Model for LoadingProcess {}

impl Default for LoadingProcess {
    fn default() -> Self {
        LoadingProcess {
            element_name: "LoadingProcess".into(),
            element_type: "LoadingProcess".into(),
            req_material_upstream: Requestor::new(),
            withdraw_material_upstream: Requestor::new(),
            req_trucks_upstream: Requestor::new(),
            withdraw_trucks_upstream: Requestor::new(),
            push_downstream: Output::new(),
        }
    }
}

pub struct TruckAndOreQueue {
    queue: Vec<TruckAndOre>,
}

impl ResourceAdd<Vec<TruckAndOre>> for TruckAndOreQueue {
    fn add(&mut self, mut truck_and_ore: Vec<TruckAndOre>) {
        while !truck_and_ore.is_empty() {
            let item = truck_and_ore.pop().unwrap();
            self.queue.push(item);
        }
    }
}

impl ResourceRemove<i32, Vec<TruckAndOre>> for TruckAndOreQueue {
    fn sub(&mut self, other: i32) -> Vec<TruckAndOre> {
        let mut removed_items = vec![];
        for _ in 0..other {
            if let Some(item) = self.queue.pop() {
                removed_items.push(item);
            } else {
                break;
            }
        }
        removed_items
    }
}

impl Default for TruckAndOreQueue {
    fn default() -> Self {
        TruckAndOreQueue { queue: vec![] }
    }
}

impl LoadingProcess {
    pub fn with_name(mut self, name: String) -> Self {
        self.element_name = name;
        self
    }

    pub fn check_update_state<'a>(
        &'a mut self,
        notif_meta: NotificationMetadata,
        cx: &'a mut Context<Self>,
    ) -> impl Future<Output = ()> + 'a {
        async move {
            let truck_id = *self.withdraw_trucks_upstream.send((1, NotificationMetadata {
                time: notif_meta.time,
                element_from: self.element_name.clone(),
                message: "Truck request".into(),
            })).await.next().unwrap().first().unwrap();
            let material = self.withdraw_material_upstream.send((1., NotificationMetadata {
                time: notif_meta.time,
                element_from: self.element_name.clone(),
                message: "Material request".into(),
            })).await.next().unwrap();
            let truck_and_ore = vec![TruckAndOre { truck: truck_id, ore: material }];
            self.push_downstream.send((truck_and_ore, NotificationMetadata {
                time: notif_meta.time,
                element_from: self.element_name.clone(),
                message: "Truck and ore".into(),
            })).await;
        }
    }
}

struct DumpingProcess {
    element_name: String,
    element_type: String,
    req_upstream: Requestor<(), TruckAndOre>,
    withdraw_upstream: Requestor<i32, Vec<i32>>,

    req_trucks_downstream: Requestor<(), QueueState>,
    push_trucks_downstream: Output<i32>,
    req_material_downstream: Requestor<(), ArrayStockState>,
    push_material_downstream: Output<ArrayResource>,
}

impl Model for DumpingProcess {}

impl Default for DumpingProcess {
    fn default() -> Self {
        DumpingProcess {
            element_name: "DumpingProcess".into(),
            element_type: "DumpingProcess".into(),
            req_upstream: Requestor::new(),
            withdraw_upstream: Requestor::new(),
            req_trucks_downstream: Requestor::new(),
            push_trucks_downstream: Output::new(),
            req_material_downstream: Requestor::new(),
            push_material_downstream: Output::new(),
        }
    }
}

impl DumpingProcess {
    fn with_name(mut self, name: String) -> Self {
        self.element_name = name;
        self
    }
}

define_stock!(
    /// LoadedHaulStock
    name = LoadedHaulStock,
    resource_type = TruckAndOreQueue,
    initial_resource = Default::default(),
    add_type = Vec<TruckAndOre>,
    remove_type = Vec<TruckAndOre>,
    remove_parameter_type = i32,
    state_type = QueueState,
    fields = {
        low_capacity: f64,
        max_capacity: f64
    },
    get_state_method = |x: &Self| -> QueueState {
        QueueState::Normal {
            occupied: x.resource.queue.len() as i32,
            empty: i32::MAX,
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
                occupied: x.resource.queue.len() as i32,
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
    let process_logger = EventLogger::<ArrayProcessLog>::new(100_000);

    let source_stockpile = ArrayStock::new()
        .with_name("SourceStockpile".into())
        .with_log_consumer(&stock_logger);
    let source_stockpile_mbox: Mailbox<ArrayStock> = Mailbox::new();
    let source_stockpile_addr = source_stockpile_mbox.address();

    let truck_stock = MyQueueStock::new()
        .with_name("TruckStock".into())
        .with_log_consumer(&queue_logger);
    let truck_stock_mbox: Mailbox<MyQueueStock> = Mailbox::new();
    let truck_stock_addr = truck_stock_mbox.address();

    let mut loading_process = LoadingProcess::default().with_name("LoadingProcess".into());
    let loading_mbox: Mailbox<LoadingProcess> = Mailbox::new();
    let loading_addr = loading_mbox.address();

    let loaded_trucks = LoadedHaulStock::new()
        .with_name("LoadedTrucks".into())
        .with_log_consumer(&stock_logger_2);
    let loaded_trucks_mbox: Mailbox<LoadedHaulStock> = Mailbox::new();
    let loaded_trucks_addr = loaded_trucks_mbox.address();

    // source_stockpile.connect(LoadingProcess::withdraw_material_upstream, &loading_addr);
    loading_process.req_material_upstream.connect(ArrayStock::get_state, &source_stockpile_addr);
    loading_process.req_trucks_upstream.connect(MyQueueStock::get_state, &truck_stock_addr);
    loading_process.withdraw_material_upstream.connect(ArrayStock::remove, &source_stockpile_addr);
    loading_process.withdraw_trucks_upstream.connect(MyQueueStock::remove, &truck_stock_addr);
    loading_process.push_downstream.connect(LoadedHaulStock::add, &loaded_trucks_addr);

    let sim_init = SimInit::new()
        .add_model(source_stockpile, source_stockpile_mbox, "SourceStockpile")
        .add_model(truck_stock, truck_stock_mbox, "TruckStock")
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

}
