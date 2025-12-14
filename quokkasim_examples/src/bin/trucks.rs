// Goal: Have a component to represent trucks or similar?

/*
 * Processing area is parameterised by:
 * - Number of trucks
 * - Cycle time per route
 * - Route allocation
 * - Payload?
 */

use std::time::Duration;

use quokkasim::{components::mixed::{DefaultLoadingProcess, DefaultUnloadingProcess, LoadResource}, prelude::{Connect, Connection, ContResource, ContStockLog, ContStockState, DefaultContStock, DefaultDiscProcess, DefaultDiscSink, DefaultDiscStock, DiscProcessLog, DiscStock, DiscStockLog, DiscStockState, DiscreteArithmetic, DistributionFactory, EventQueue, MonotonicTime, Projectable, SimInit}};
use serde::Serialize;

#[derive(Clone, Debug, Serialize, Default)]
struct Truck<ResourceType> where ResourceType: ContResource + Projectable<f64> {
    element_name: String,
    element_type: String,
    element_code: String,
    capacity: f64,
    contents: Option<ResourceType>
}

impl<ResourceType> LoadResource<ResourceType> for Truck<ResourceType> where ResourceType: ContResource + Projectable<f64>  {
    fn load_resource(&mut self, resource: ResourceType) {
        self.contents = Some(resource);
    }

    fn unload_resource(&mut self, quantity: f64) -> ResourceType {
        match self.contents.as_mut() {
            Some(res) => {
                if res.total() <= quantity {
                    return self.unload_resource_all().unwrap_or_default()
                } else {
                    let moved = res.remove(quantity);
                    return moved
                }
            },
            None => {
                ResourceType::default()
            }
        }
    }

    fn unload_resource_all(&mut self) -> Option<ResourceType> {
        self.contents.take()
    }
}

fn main() {
    let mut df = DistributionFactory::new(12345);

    let mut stockpile_1: DefaultContStock<f64, ContStockState, ContStockLog<f64>> = DefaultContStock::new()
        .with_name("Stockpile1")
        .with_low_capacity(1.)
        .with_max_capacity(500.)
        .with_initial_resource(1000.);
    let (stockpile_1_mailbox, mut stockpile_1_address) = stockpile_1.create_mailbox();

    let mut truck_loading_queue: DefaultDiscStock<Truck<f64>, DiscStockState, DiscStockLog<Truck<f64>>> = DefaultDiscStock::new()
        .with_name("TruckLoadingQueue")
        .with_low_capacity(0)
        .with_max_capacity(10)
        .with_initial_resources(
            (0..5)
                .map(|i| Truck {
                    element_name: format!("Truck{}", i),
                    element_type: "DumpTruck".to_string(),
                    element_code: format!("DT-{}", i),
                    capacity: 100.0,
                    contents: None,
                })
                .collect(),
        );
    let (truck_loading_queue_mailbox, mut truck_loading_queue_address) = truck_loading_queue.create_mailbox();

    let mut truck_loading: DefaultLoadingProcess<Truck<f64>, DiscProcessLog<Truck<f64>>, f64> = DefaultLoadingProcess::new()
        .with_name("TruckLoading")
        .with_process_time_distr(
            df.create(quokkasim::distributions::DistributionConfig::Constant(15.0))
                .unwrap(),
        )
        .with_process_quantity_distr(
            df.create(quokkasim::distributions::DistributionConfig::Constant(1.0))
                .unwrap(),
        );
    let (truck_loading_mailbox, mut truck_loading_address) = truck_loading.create_mailbox();
    let mut loaded_trucks_queue: DefaultDiscStock<Truck<f64>, DiscStockState, DiscStockLog<Truck<f64>>> = DefaultDiscStock::new()
        .with_name("LoadedTrucksQueue")
        .with_low_capacity(0)
        .with_max_capacity(10);
    let (loaded_trucks_queue_mailbox, mut loaded_trucks_queue_address) = loaded_trucks_queue.create_mailbox();
    let mut loaded_trucks_travel: DefaultDiscProcess<Truck<f64>, DiscProcessLog<Truck<f64>>> = DefaultDiscProcess::new()
        .with_name("LoadedTrucksTravel")
        .with_process_time_distr(
            df.create(quokkasim::distributions::DistributionConfig::Constant(60.0))
                .unwrap(),
        )
        .with_process_quantity_distr(
            df.create(quokkasim::distributions::DistributionConfig::Constant(1.0))
                .unwrap(),
        );
    let (loaded_trucks_travel_mailbox, mut loaded_trucks_travel_address) = loaded_trucks_travel.create_mailbox();
    let mut truck_unloading_queue: DefaultDiscStock<Truck<f64>, DiscStockState, DiscStockLog<Truck<f64>>> = DefaultDiscStock::new()
        .with_name("TruckUnloadingQueue")
        .with_low_capacity(0)
        .with_max_capacity(10);
    let (truck_unloading_queue_mailbox, mut truck_unloading_queue_address) = truck_unloading_queue.create_mailbox();
    let mut truck_unloading: DefaultUnloadingProcess<Truck<f64>, DiscProcessLog<Truck<f64>>, f64> = DefaultUnloadingProcess::new()
        .with_name("TruckUnloading")
        .with_process_time_distr(
            df.create(quokkasim::distributions::DistributionConfig::Constant(1.0))
                .unwrap(),
        );
    let (truck_unloading_mailbox, mut truck_unloading_address) = truck_unloading.create_mailbox();
    let mut truck_unloading_queue: DefaultDiscStock<Truck<f64>, DiscStockState, DiscStockLog<Truck<f64>>> = DefaultDiscStock::new()
        .with_name("TruckUnloadingQueue")
        .with_low_capacity(0)
        .with_max_capacity(10);
    let (truck_unloading_queue_mailbox, mut truck_unloading_queue_address) = truck_unloading_queue.create_mailbox();
    let mut truck_unloading: DefaultUnloadingProcess<Truck<f64>, DiscProcessLog<Truck<f64>>, f64> = DefaultUnloadingProcess::new()
        .with_name("TruckUnloading")
        .with_process_time_distr(
            df.create(quokkasim::distributions::DistributionConfig::Constant(15.0))
                .unwrap(),
        )
        .with_process_quantity_distr(
            df.create(quokkasim::distributions::DistributionConfig::Constant(1.0))
                .unwrap(),
        );
    let (truck_unloading_mailbox, mut truck_unloading_address) = truck_unloading.create_mailbox();
    let mut empty_trucks_queue: DefaultDiscStock<Truck<f64>, DiscStockState, DiscStockLog<Truck<f64>>> = DefaultDiscStock::new()
        .with_name("EmptyTrucksQueue")
        .with_low_capacity(0)
        .with_max_capacity(10);
    let (empty_trucks_queue_mailbox, mut empty_trucks_queue_address) = empty_trucks_queue.create_mailbox();
    let mut empty_trucks_travel: DefaultDiscProcess<Truck<f64>, DiscProcessLog<Truck<f64>>> = DefaultDiscProcess::new()
        .with_name("EmptyTrucksTravel")
        .with_process_time_distr(
            df.create(quokkasim::distributions::DistributionConfig::Constant(1.0))
                .unwrap(),
        );
    let (empty_trucks_travel_mailbox, mut empty_trucks_travel_address) = empty_trucks_travel.create_mailbox();
    let mut empty_trucks_queue: DefaultDiscStock<Truck<f64>, DiscStockState, DiscStockLog<Truck<f64>>> = DefaultDiscStock::new()
        .with_name("EmptyTrucksQueue")
        .with_low_capacity(0)
        .with_max_capacity(10);
    let (empty_trucks_queue_mailbox, mut empty_trucks_queue_address) = empty_trucks_queue.create_mailbox();
    let mut empty_trucks_travel: DefaultDiscProcess<Truck<f64>, DiscProcessLog<Truck<f64>>> = DefaultDiscProcess::new()
        .with_name("EmptyTrucksTravel")
        .with_process_time_distr(
            df.create(quokkasim::distributions::DistributionConfig::Constant(60.0))
                .unwrap(),
        )
        .with_process_quantity_distr(
            df.create(quokkasim::distributions::DistributionConfig::Constant(1.0))
                .unwrap(),
        );
    let (empty_trucks_travel_mailbox, mut empty_trucks_travel_address) = empty_trucks_travel.create_mailbox();
    let mut stockpile_2: DefaultContStock<f64, ContStockState, ContStockLog<f64>> = DefaultContStock::new()
        .with_name("Stockpile2")
        .with_low_capacity(1.)
        .with_max_capacity(500.); 
    let (stockpile_2_mailbox, mut stockpile_2_address) = stockpile_2.create_mailbox();

    // Connections
    let mut c = Connection;

    c.connect(
        (&mut stockpile_1, &stockpile_1_address, None),
        (&mut truck_loading, &truck_loading_address, None),
    ).unwrap();  

    c.connect(
        (&mut truck_loading_queue, &truck_loading_queue_address, None),
        (&mut truck_loading, &truck_loading_address, None),
    ).unwrap();

    c.connect(
        (&mut truck_loading, &truck_loading_address, None),
        (&mut loaded_trucks_queue, &loaded_trucks_queue_address, None),
    ).unwrap();

    c.connect(
        (&mut loaded_trucks_queue, &loaded_trucks_queue_address, None),
        (&mut loaded_trucks_travel, &loaded_trucks_travel_address, None),
    ).unwrap();

    c.connect(
        (&mut loaded_trucks_travel, &loaded_trucks_travel_address, None),
        (&mut truck_unloading_queue, &truck_unloading_queue_address, None),
    ).unwrap();

    c.connect(
        (&mut truck_unloading_queue, &truck_unloading_queue_address, None),
        (&mut truck_unloading, &truck_unloading_address, None),
    ).unwrap();

    c.connect(
        (&mut truck_unloading, &truck_unloading_address, None),
        (&mut empty_trucks_queue, &empty_trucks_queue_address, None),
    ).unwrap();

    c.connect(
        (&mut truck_unloading, &truck_unloading_address, None),
        (&mut stockpile_2, &stockpile_2_address, None),
    ).unwrap();

    c.connect(
        (&mut empty_trucks_queue, &empty_trucks_queue_address, None),
        (&mut empty_trucks_travel, &empty_trucks_travel_address, None),
    ).unwrap();

    c.connect(
        (&mut empty_trucks_travel, &empty_trucks_travel_address, None),
        (&mut truck_loading_queue, &truck_loading_queue_address, None),
    ).unwrap();

    // Loggers

    let process_logger = EventQueue::<DiscProcessLog<Truck<f64>>>::new();
    truck_loading.log_emitter.connect_sink(&process_logger);
    loaded_trucks_travel.log_emitter.connect_sink(&process_logger);
    truck_unloading.log_emitter.connect_sink(&process_logger);
    empty_trucks_travel.log_emitter.connect_sink(&process_logger);

    let truck_stock_logger = EventQueue::<DiscStockLog<Truck<f64>>>::new();
    truck_loading_queue.log_emitter.connect_sink(&truck_stock_logger);
    loaded_trucks_queue.log_emitter.connect_sink(&truck_stock_logger);
    truck_unloading_queue.log_emitter.connect_sink(&truck_stock_logger);
    empty_trucks_queue.log_emitter.connect_sink(&truck_stock_logger);

    let stockpile_logger = EventQueue::<ContStockLog<f64>>::new();
    stockpile_1.log_emitter.connect_sink(&stockpile_logger);
    stockpile_2.log_emitter.connect_sink(&stockpile_logger);

    // Simulation setup

    let sim_init = SimInit::new()
        .add_model(stockpile_1, stockpile_1_mailbox, "Stockpile1")
        .add_model(truck_loading_queue, truck_loading_queue_mailbox, "TruckLoadingQueue")
        .add_model(truck_loading, truck_loading_mailbox, "TruckLoading")
        .add_model(loaded_trucks_queue, loaded_trucks_queue_mailbox, "LoadedTrucksQueue")
        .add_model(loaded_trucks_travel, loaded_trucks_travel_mailbox, "LoadedTrucksTravel")
        .add_model(truck_unloading_queue, truck_unloading_queue_mailbox, "TruckUnloadingQueue")
        .add_model(truck_unloading, truck_unloading_mailbox, "TruckUnloading")
        .add_model(empty_trucks_queue, empty_trucks_queue_mailbox, "EmptyTrucksQueue")
        .add_model(empty_trucks_travel, empty_trucks_travel_mailbox, "EmptyTrucksTravel")
        .add_model(stockpile_2, stockpile_2_mailbox, "Stockpile2");
    let start_time = MonotonicTime::try_from_date_time(2025, 1, 1, 0, 0, 0, 0).unwrap();
    let duration = Duration::from_secs(3600);
    let (mut sim, _) = sim_init.init(start_time).unwrap();
    sim.step_until(start_time + duration).unwrap();

    for log in process_logger.into_reader() {
        println!("{:?}", log);
    }
}