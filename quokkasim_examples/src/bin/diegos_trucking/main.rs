use std::{error::Error, fs::create_dir_all, time::Duration};
use quokkasim::nexosim::Mailbox;
use quokkasim::prelude::*;
use quokkasim::define_model_enums;

mod iron_ore;
use iron_ore::*;

mod truck;
use truck::*;

mod loading;
use loading::*;

mod dumping; 
use dumping::*;

mod loggers;
use loggers::*;

define_model_enums! {
    pub enum ComponentModel {
        IronOreStock(VectorStock<IronOre>, Mailbox<VectorStock<IronOre>>),
        DiscreteStockTruck(DiscreteStock<Truck>, Mailbox<DiscreteStock<Truck>>),
        LoadingProcess(LoadingProcess, Mailbox<LoadingProcess>),
        DumpingProcess(DumpingProcess, Mailbox<DumpingProcess>),
        DiscreteParallelProcessTruck(DiscreteParallelProcess<Option<Truck>, (), Option<Truck>>, Mailbox<DiscreteParallelProcess<Option<Truck>, (), Option<Truck>>>),
    }
    pub enum ComponentModelAddress {}
    pub enum ComponentLogger {
        TruckingProcessLogger(TruckingProcessLogger),
        TruckStockLogger(DiscreteStockLogger<Truck>),
        IronOreStockLogger(VectorStockLogger<IronOre>),
    }
    pub enum ScheduledEvent {}
}

impl CustomComponentConnection for ComponentModel {
    fn connect_components(a: &mut Self, b: &mut Self, n: Option<usize>) -> Result<(), Box<dyn Error>> {
        match (a, b) {
            (ComponentModel::IronOreStock(ore_stock, ore_stock_mbox), ComponentModel::LoadingProcess(load_process, load_process_mbox)) => {
                load_process.req_upstream_ore.connect(VectorStock::<IronOre>::get_state_async, ore_stock_mbox.address());
                load_process.withdraw_upstream_ore.connect(VectorStock::<IronOre>::remove, ore_stock_mbox.address());
                ore_stock.state_emitter.connect(LoadingProcess::update_state, load_process_mbox.address());
                Ok(())
            },
            (ComponentModel::DiscreteStockTruck(truck_stock, truck_stock_mbox), ComponentModel::LoadingProcess(load_process, load_process_mbox)) => {
                load_process.req_upstream_trucks.connect(DiscreteStock::<Truck>::get_state_async, truck_stock_mbox.address());
                load_process.withdraw_upstream_trucks.connect(DiscreteStock::<Truck>::remove, truck_stock_mbox.address());
                truck_stock.state_emitter.connect(LoadingProcess::update_state, load_process_mbox.address());
                Ok(())
            },
            (ComponentModel::LoadingProcess(load_process, load_process_mbox), ComponentModel::DiscreteStockTruck(truck_stock, truck_stock_mbox)) => {
                load_process.req_downstream_trucks.connect(DiscreteStock::<Truck>::get_state_async, truck_stock_mbox.address());
                load_process.push_downstream_trucks.connect(DiscreteStock::<Truck>::add, truck_stock_mbox.address());
                truck_stock.state_emitter.connect(LoadingProcess::update_state, load_process_mbox.address());
                Ok(())
            },
            (ComponentModel::DiscreteStockTruck(truck_stock, truck_stock_mbox), ComponentModel::DiscreteParallelProcessTruck(process, process_mbox)) => {
                process.req_upstream.connect(DiscreteStock::<Truck>::get_state_async, truck_stock_mbox.address());
                process.withdraw_upstream.connect(DiscreteStock::<Truck>::remove, truck_stock_mbox.address());
                truck_stock.state_emitter.connect(DiscreteParallelProcess::update_state, process_mbox.address());
                Ok(())
            },
            (ComponentModel::DiscreteParallelProcessTruck(process, process_mbox), ComponentModel::DiscreteStockTruck(ready_to_dump, ready_to_dump_mbox)) => {
                process.req_downstream.connect(DiscreteStock::<Truck>::get_state_async, ready_to_dump_mbox.address());
                process.push_downstream.connect(DiscreteStock::<Truck>::add, ready_to_dump_mbox.address());
                ready_to_dump.state_emitter.connect(DiscreteParallelProcess::update_state, process_mbox.address());
                Ok(())
            },
            (ComponentModel::DiscreteStockTruck(truck_stock, truck_stock_mbox), ComponentModel::DumpingProcess(dump_process, dump_process_mbox)) => {
                dump_process.req_upstream_trucks.connect(DiscreteStock::<Truck>::get_state_async, truck_stock_mbox.address());
                dump_process.withdraw_upstream_trucks.connect(DiscreteStock::<Truck>::remove, truck_stock_mbox.address());
                truck_stock.state_emitter.connect(DumpingProcess::update_state, dump_process_mbox.address());
                Ok(())
            },
            (ComponentModel::DumpingProcess(dump_process, dump_process_mbox), ComponentModel::DiscreteStockTruck(truck_stock, truck_stock_mbox)) => {
                dump_process.req_downstream_trucks.connect(DiscreteStock::<Truck>::get_state_async, truck_stock_mbox.address());
                dump_process.push_downstream_trucks.connect(DiscreteStock::<Truck>::add, truck_stock_mbox.address());
                truck_stock.state_emitter.connect(DumpingProcess::update_state, dump_process_mbox.address());
                Ok(())
            },
            (ComponentModel::DumpingProcess(dump_process, dump_process_mbox), ComponentModel::IronOreStock(ore_stock, ore_stock_mbox)) => {
                dump_process.req_downstream_ore.connect(VectorStock::<IronOre>::get_state_async, ore_stock_mbox.address());
                dump_process.push_downstream_ore.connect(VectorStock::<IronOre>::add, ore_stock_mbox.address());
                ore_stock.state_emitter.connect(DumpingProcess::update_state, dump_process_mbox.address());
                Ok(())
            },
            (a, b) => Err(format!("No component connection defined from {} to {} (n={:?})", a, b, n).into()),
        }
    }
}

impl CustomLoggerConnection for ComponentLogger { 
    type ComponentType = ComponentModel;
    fn connect_logger(a: &mut Self, b: &mut Self::ComponentType, n: Option<usize>) -> Result<(), Box<dyn Error>> {
        match (a, b, n) {
            (ComponentLogger::TruckingProcessLogger(logger), ComponentModel::LoadingProcess(process, _), _) => {
                process.log_emitter.connect_sink(&logger.buffer); 
                Ok(())
            },
            (ComponentLogger::IronOreStockLogger(logger), ComponentModel::IronOreStock(stock, _), _) => {
                stock.log_emitter.connect_sink(&logger.buffer);
                Ok(())
            },
            (ComponentLogger::TruckStockLogger(logger), ComponentModel::DiscreteStockTruck(stock, _), _) => {
                stock.log_emitter.connect_sink(&logger.buffer);
                Ok(())
            },
            (ComponentLogger::TruckingProcessLogger(logger), ComponentModel::DiscreteParallelProcessTruck(process, _), _) => {
                // process.log_emitter.connect_sink(&logger.buffer);
                process.log_emitter.filter_map_connect_sink(|x| {
                    let event = match &x.event {
                        DiscreteProcessLogType::ProcessStart { resource } => TruckingProcessLogType::TruckMovementStart { truck_id: resource.clone().unwrap().truck_id },
                        DiscreteProcessLogType::ProcessSuccess { resource } => TruckingProcessLogType::TruckMovementSuccess { truck_id: resource.clone().unwrap().truck_id },
                        DiscreteProcessLogType::ProcessFailure { reason } => TruckingProcessLogType::TruckMovementFailure { reason },
                    };
                    Some(TruckingProcessLog {
                        element_name: x.element_name.clone(),
                        element_type: x.element_type.clone(),
                        event_id: x.event_id,
                        time: x.time.clone(),
                        event,
                    })
                }, &logger.buffer);
                // TODO: Implement logging for DiscreteParallelProcess
                Ok(())
            },
            (ComponentLogger::TruckingProcessLogger(logger), ComponentModel::DumpingProcess(process, _), _) => {
                process.log_emitter.connect_sink(&logger.buffer);
                Ok(())
            },
            (a, b, _) => Err(format!("No logger connection defined from {} to {} (n={:?})", a, b, n).into()),
        }
    }
}

impl CustomInit for ComponentModelAddress {
    fn initialise(&mut self, simu: &mut Simulation) -> Result<(), ExecutionError> {
        let notif_meta = NotificationMetadata {
            time: simu.time(),
            element_from: "Init".into(),
            message: "Start".into(),
        };
        match self {
            ComponentModelAddress::LoadingProcess(addr) => {
                simu.process_event(LoadingProcess::update_state, notif_meta.clone(), addr.clone()).unwrap();
                Ok(())
            },
            ComponentModelAddress::DiscreteParallelProcessTruck(addr) => {
                simu.process_event(DiscreteParallelProcess::update_state, notif_meta.clone(), addr.clone())?;
                Ok(())
            },
            ComponentModelAddress::DumpingProcess(addr) => {
                simu.process_event(DumpingProcess::update_state, notif_meta.clone(), addr.clone())?;
                Ok(())
            },
            x => {
                panic!("No initialisation defined for component address: {}", x);
            }
        }
    }
}

fn main() {

    let base_seed = 987654321;
    let mut df = DistributionFactory::new(base_seed);

    let mut source_sp = ComponentModel::IronOreStock(VectorStock::new()
        .with_name("source_sp".into())
        .with_type("IronOreStock".into())
        .with_initial_vector(IronOre { fe: 60., other_elements: 40., magnetite: 10., hematite: 5., limonite: 15. })
        .with_low_capacity(10.)
        .with_max_capacity(100.),
        Mailbox::new(),
    );

    let mut trucks_ready_to_load = ComponentModel::DiscreteStockTruck(DiscreteStock::new()
        .with_name("trucks_ready_to_load".into())
        .with_type("TruckStock".into())
        .with_initial_contents(vec![
            Truck { ore: None, truck_id: "Truck_01".into() },
            // Truck { ore: None, truck_id: "Truck_02".into() },
            // Truck { ore: None, truck_id: "Truck_03".into() },
        ])
        .with_low_capacity(0)
        .with_max_capacity(10),
        Mailbox::new(),
    );

    let mut loading_process = ComponentModel::LoadingProcess(LoadingProcess::new()
        .with_name("loading_process".into())
        .with_type("LoadingProcess".into())
        .with_process_quantity_distr(df.create(DistributionConfig::Constant(5.)).unwrap())
        .with_process_time_distr(df.create(DistributionConfig::Exponential { mean: 10. }).unwrap()),
        Mailbox::new(),
    );
    let mut loading_process_addr = loading_process.get_address();

    let mut trucks_loaded = ComponentModel::DiscreteStockTruck(DiscreteStock::new()
        .with_name("trucks_loaded".into())
        .with_type("TruckStock".into())
        .with_initial_contents(Vec::new())
        .with_low_capacity(0)
        .with_max_capacity(10),
        Mailbox::new(),
    );

    let mut loaded_truck_movements = ComponentModel::DiscreteParallelProcessTruck(DiscreteParallelProcess::new()
        .with_name("loaded_truck_movements".into())
        .with_type("LoadedTruckMovements".into())
        .with_process_time_distr(df.create(DistributionConfig::Exponential { mean: 120. }).unwrap()),
        Mailbox::new(),
    );
    let mut loaded_truck_movements_addr = loaded_truck_movements.get_address();

    let mut trucks_ready_to_dump = ComponentModel::DiscreteStockTruck(DiscreteStock::new()
        .with_name("trucks_ready_to_dump".into())
        .with_type("TruckStock".into())
        .with_initial_contents(Vec::new())
        .with_low_capacity(0)
        .with_max_capacity(10),
        Mailbox::new(),
    );

    let mut destination_sp = ComponentModel::IronOreStock(VectorStock::new()
        .with_name("destination_sp".into())
        .with_type("IronOreStock".into())
        .with_initial_vector(IronOre { fe: 60., other_elements: 40., magnetite: 10., hematite: 5., limonite: 15. })
        .with_low_capacity(10.)
        .with_max_capacity(999_999_999.),
        Mailbox::new(),
    );

    let mut dumping_process = ComponentModel::DumpingProcess(DumpingProcess::new()
        .with_name("dumping_process".into())
        .with_type("DumpingProcess".into())
        .with_process_quantity_distr(df.create(DistributionConfig::Constant(5.)).unwrap())
        .with_process_time_distr(df.create(DistributionConfig::Exponential { mean: 10. }).unwrap()),
        Mailbox::new(),
    );
    let mut dumping_process_addr = dumping_process.get_address();

    let mut trucks_dumped = ComponentModel::DiscreteStockTruck(DiscreteStock::new()
        .with_name("trucks_dumped".into())
        .with_type("TruckStock".into())
        .with_initial_contents(Vec::new())
        .with_low_capacity(0)
        .with_max_capacity(10),
        Mailbox::new(),
    );

    let mut dumped_truck_movements = ComponentModel::DiscreteParallelProcessTruck(DiscreteParallelProcess::new()
        .with_name("dumped_truck_movements".into())
        .with_type("DumpedTruckMovements".into())
        .with_process_time_distr(df.create(DistributionConfig::Exponential { mean: 120. }).unwrap()),
        Mailbox::new(),
    );
    let mut dumped_truck_movements_addr = dumped_truck_movements.get_address();


    // Connect components

    connect_components!(&mut source_sp, &mut loading_process).unwrap();
    connect_components!(&mut trucks_ready_to_load, &mut loading_process).unwrap();
    connect_components!(&mut loading_process, &mut trucks_loaded).unwrap();
    connect_components!(&mut trucks_loaded, &mut loaded_truck_movements).unwrap();
    connect_components!(&mut loaded_truck_movements, &mut trucks_ready_to_dump).unwrap();
    connect_components!(&mut trucks_ready_to_dump, &mut dumping_process).unwrap();
    connect_components!(&mut dumping_process, &mut destination_sp).unwrap();
    connect_components!(&mut dumping_process, &mut trucks_dumped).unwrap();
    connect_components!(&mut trucks_dumped, &mut dumped_truck_movements).unwrap();
    connect_components!(&mut dumped_truck_movements, &mut trucks_ready_to_load).unwrap();

    let mut ore_stock_logger = ComponentLogger::IronOreStockLogger(VectorStockLogger::new("OreStockLogger".into()));
    let mut truck_stock_logger = ComponentLogger::TruckStockLogger(DiscreteStockLogger::new("TruckStockLogger".into()));
    let mut truck_process_logger = ComponentLogger::TruckingProcessLogger(TruckingProcessLogger::new("TruckProcessLogger".into()));

    connect_logger!(&mut ore_stock_logger, &mut source_sp).unwrap();
    connect_logger!(&mut ore_stock_logger, &mut destination_sp).unwrap();
    connect_logger!(&mut truck_stock_logger, &mut trucks_ready_to_load).unwrap();
    connect_logger!(&mut truck_stock_logger, &mut trucks_loaded).unwrap();
    connect_logger!(&mut truck_stock_logger, &mut trucks_ready_to_dump).unwrap();
    connect_logger!(&mut truck_stock_logger, &mut trucks_dumped).unwrap();
    connect_logger!(&mut truck_process_logger, &mut loading_process).unwrap();
    connect_logger!(&mut truck_process_logger, &mut loaded_truck_movements).unwrap();
    connect_logger!(&mut truck_process_logger, &mut dumping_process).unwrap();
    connect_logger!(&mut truck_process_logger, &mut dumped_truck_movements).unwrap();

    let mut sim_builder = SimInit::new();
    sim_builder = register_component!(sim_builder, source_sp);
    sim_builder = register_component!(sim_builder, trucks_ready_to_load);
    sim_builder = register_component!(sim_builder, loading_process);
    sim_builder = register_component!(sim_builder, trucks_loaded);
    sim_builder = register_component!(sim_builder, loaded_truck_movements);
    sim_builder = register_component!(sim_builder, trucks_ready_to_dump);
    sim_builder = register_component!(sim_builder, dumping_process);
    sim_builder = register_component!(sim_builder, destination_sp);
    sim_builder = register_component!(sim_builder, trucks_dumped);
    sim_builder = register_component!(sim_builder, dumped_truck_movements);



    let start_time = MonotonicTime::try_from_date_time(2025, 5, 1, 0, 0, 0, 0).unwrap();
    let mut simu = sim_builder.init(start_time.clone()).unwrap().0;

    loading_process_addr.initialise(&mut simu).unwrap();
    loaded_truck_movements_addr.initialise(&mut simu).unwrap();
    dumping_process_addr.initialise(&mut simu).unwrap();
    dumped_truck_movements_addr.initialise(&mut simu).unwrap();

    simu.step_until(start_time + Duration::from_secs_f64(120.)).unwrap();
    // simu.step_until(start_time + Duration::from_secs_f64(9.952383953)).unwrap();

    let output_dir = "outputs/diegos_trucking";
    create_dir_all(output_dir).unwrap();

    ore_stock_logger.write_csv(output_dir.into()).unwrap();
    truck_stock_logger.write_csv(output_dir.into()).unwrap();
    truck_process_logger.write_csv(output_dir.into()).unwrap();

}
