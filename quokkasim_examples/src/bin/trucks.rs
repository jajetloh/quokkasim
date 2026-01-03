use std::time::Duration;
use quokkasim::{
    components::mixed::{DefaultLoadingProcess, DefaultUnloadingProcess, LoadResource},
    prelude::{Connect, Connection, ContResource, ContStockLog, ContStockState, DefaultContStock, DefaultDiscStock, DefaultTravelProcess, DiscProcessLog, DiscStockLog, DiscStockState, DistributionConfig, DistributionFactory, EventQueue, MonotonicTime, Projectable, SimInit
    }
};
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
                    let moved = self.unload_resource_all();
                    if moved.is_none() {
                        return ResourceType::default();
                    } else {
                        return moved.unwrap();
                    }
                } else {
                    let moved = res.remove(quantity);
                    return moved;
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
        .with_code("SP1")
        .with_low_capacity(1.)
        .with_max_capacity(500.)
        .with_initial_resource(1000.);
    let (stockpile_1_mailbox, stockpile_1_address) = stockpile_1.create_mailbox();

    let mut truck_loading_queue: DefaultDiscStock<Truck<f64>, DiscStockState, DiscStockLog<Truck<f64>>> = DefaultDiscStock::new()
        .with_name("TruckLoadingQueue")
        .with_code("PreLQ")
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
    let (truck_loading_queue_mailbox, truck_loading_queue_address) = truck_loading_queue.create_mailbox();

    let mut truck_loading: DefaultLoadingProcess<Truck<f64>, DiscProcessLog<Truck<f64>>, f64> = DefaultLoadingProcess::new()
        .with_name("TruckLoading")
        .with_code("L")
        .with_process_time_distr(
            df.create(DistributionConfig::Constant(15.0))
                .unwrap(),
        )
        .with_process_quantity_distr(
            df.create(DistributionConfig::Constant(1.0))
                .unwrap(),
        );
    let (truck_loading_mailbox, truck_loading_address) = truck_loading.create_mailbox();

    let mut truck_travel_manager: DefaultTravelProcess<Truck<f64>, DiscProcessLog<Truck<f64>>> = DefaultTravelProcess::new()
        .with_name("TruckTravelManager")
        .with_code("TTM");
    let (truck_travel_manager_mailbox, truck_travel_manager_address) = truck_travel_manager.create_mailbox();
    
    let mut loaded_travel_distr = df.create(DistributionConfig::Uniform { min: 60., max: 120. }).unwrap();
    let mut empty_travel_distr = df.create(DistributionConfig::Uniform { min: 30., max: 90. }).unwrap();
    truck_travel_manager.travel_routing_fn = Box::new(move |p, truck| {
        match truck.contents {
            Some(_) => {
                let travel_time = loaded_travel_distr.sample();
                (Duration::from_secs_f64(travel_time), "PreUQ".into())
            },
            None => {
                let travel_time = empty_travel_distr.sample();
                (Duration::from_secs_f64(travel_time), "PreLQ".into())
            }
        }
    });

    let mut truck_unloading_queue: DefaultDiscStock<Truck<f64>, DiscStockState, DiscStockLog<Truck<f64>>> = DefaultDiscStock::new()
        .with_name("TruckUnloadingQueue")
        .with_code("PreUQ")
        .with_low_capacity(0)
        .with_max_capacity(10);
    let (truck_unloading_queue_mailbox, truck_unloading_queue_address) = truck_unloading_queue.create_mailbox();
    
    let mut truck_unloading: DefaultUnloadingProcess<Truck<f64>, DiscProcessLog<Truck<f64>>, f64> = DefaultUnloadingProcess::new()
        .with_name("TruckUnloading")
        .with_code("U")
        .with_process_time_distr(
            df.create(DistributionConfig::Constant(15.0))
                .unwrap(),
        )
        .with_process_quantity_distr(
            df.create(DistributionConfig::Constant(1.0))
                .unwrap(),
        );
    let (truck_unloading_mailbox, truck_unloading_address) = truck_unloading.create_mailbox();
    
    let mut stockpile_2: DefaultContStock<f64, ContStockState, ContStockLog<f64>> = DefaultContStock::new()
        .with_name("Stockpile2")
        .with_code("SP2")
        .with_low_capacity(1.)
        .with_max_capacity(500.); 
    let (stockpile_2_mailbox, stockpile_2_address) = stockpile_2.create_mailbox();

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
        (&mut truck_travel_manager, &truck_travel_manager_address, None),
    ).unwrap();
    c.connect(
        (&mut truck_travel_manager, &truck_travel_manager_address, None),
        (&mut truck_unloading_queue, &truck_unloading_queue_address, None),
    ).unwrap();
    c.connect(
        (&mut truck_unloading_queue, &truck_unloading_queue_address, None),
        (&mut truck_unloading, &truck_unloading_address, None),
    ).unwrap();
    c.connect(
        (&mut truck_unloading, &truck_unloading_address, None),
        (&mut truck_travel_manager, &truck_travel_manager_address, None),
    ).unwrap();
    c.connect(
        (&mut truck_unloading, &truck_unloading_address, None),
        (&mut stockpile_2, &stockpile_2_address, None),
    ).unwrap();
    c.connect(
        (&mut truck_travel_manager, &truck_travel_manager_address, None),
        (&mut truck_loading_queue, &truck_loading_queue_address, None),
    ).unwrap();

    // Loggers

    let stock_logger = EventQueue::<ContStockLog<f64>>::new();
    let process_logger = EventQueue::<DiscProcessLog<Truck<f64>>>::new();
    let truck_queue_logger = EventQueue::<DiscStockLog<Truck<f64>>>::new();

    stockpile_1.log_emitter.connect_sink(&stock_logger);
    truck_loading_queue.log_emitter.connect_sink(&truck_queue_logger);
    truck_loading.log_emitter.connect_sink(&process_logger);
    truck_travel_manager.log_emitter.connect_sink(&process_logger);
    truck_unloading_queue.log_emitter.connect_sink(&truck_queue_logger);
    truck_unloading.log_emitter.connect_sink(&process_logger);
    stockpile_2.log_emitter.connect_sink(&stock_logger);

    // Model run

    let sim_init = SimInit::new()
        .add_model(stockpile_1, stockpile_1_mailbox, "Stockpile1".to_string())
        .add_model(truck_loading_queue, truck_loading_queue_mailbox, "TruckLoadingQueue".to_string())
        .add_model(truck_loading, truck_loading_mailbox, "TruckLoading".to_string())
        .add_model(truck_travel_manager, truck_travel_manager_mailbox, "TruckTravelManager".to_string())
        .add_model(truck_unloading_queue, truck_unloading_queue_mailbox, "TruckUnloadingQueue".to_string())
        .add_model(truck_unloading, truck_unloading_mailbox, "TruckUnloading".to_string())
        .add_model(stockpile_2, stockpile_2_mailbox, "Stockpile2".to_string());
    let start_time = MonotonicTime::try_from_date_time(2026, 1, 1, 0, 0, 0, 0).unwrap();
    let sim_duration = Duration::from_secs(86400); // 1 day
    let (mut simu, sched) = sim_init.init(start_time).unwrap();
    simu.step_until(start_time + sim_duration).unwrap();

    for log_entry in process_logger.into_reader() {
        println!("{:?}", log_entry);
    }
}