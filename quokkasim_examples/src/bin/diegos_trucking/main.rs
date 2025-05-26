use std::collections::VecDeque;
use std::{error::Error, fs::create_dir_all, time::Duration};
use quokkasim::nexosim::Mailbox;
use quokkasim::prelude::*;
use quokkasim::define_model_enums;
use serde::{ser::SerializeStruct, Serialize};

// mod loggers;
mod iron_ore;
use iron_ore::*;
mod truck;
use truck::*;

mod loading;
use loading::*;

mod loggers;
use loggers::*;

// #[derive(Clone, Debug)]
// struct SeqDeque {
//     deque: VecDeque<TruckAndIronOre>
// }

// impl Default for SeqDeque {
//     fn default() -> Self {
//         SeqDeque {
//             deque: VecDeque::<TruckAndIronOre>::new()
//         }
//     }
// }

// impl VectorArithmetic<Option<TruckAndIronOre>, (), u32> for SeqDeque where TruckAndIronOre: Clone {
//     fn add(&mut self, other: Option<TruckAndIronOre>) {
//         match other {
//             Some(item) => {
//                 self.deque.push_back(item);
//             }
//             None => {}
//         }
//     }

//     fn subtract_parts(&self, _: ()) -> SubtractParts<Self, Option<TruckAndIronOre>> {
//         let mut cloned = self.clone();
//         let subtracted = cloned.deque.pop_front();
//         SubtractParts {
//             remaining: cloned,
//             subtracted: subtracted
//         }
//     }

//     fn total(&self) -> u32 {
//         self.deque.len() as u32
//     }
// }

// #[derive(Clone, Debug)]
// struct TruckStock {
//     pub element_name: String,
//     pub element_type: String,
//     pub sequence: SeqDeque,
//     pub log_emitter: Output<TruckStockLog>,
//     pub state_emitter: Output<NotificationMetadata>,
//     pub low_capacity: u32,
//     pub max_capacity: u32,
//     pub prev_state: Option<TruckStockState>,
//     next_event_id: u64,
// }

// impl Default for TruckStock {
//     fn default() -> Self {
//         TruckStock {
//             element_name: "TruckStock".to_string(),
//             element_type: "TruckStock".to_string(),
//             sequence: SeqDeque::default(),
//             log_emitter: Output::new(),
//             state_emitter: Output::new(),
//             low_capacity: 0,
//             max_capacity: 1,
//             prev_state: None,
//             next_event_id: 0,
//         }
//     }
// }

// impl Stock<SeqDeque, Option<TruckAndIronOre>, (), Option<TruckAndIronOre>, u32> for TruckStock where Self: Model {
//     type StockState = TruckStockState;
//     fn get_state(&mut self) -> Self::StockState {
//         let occupied = self.sequence.total();
//         let empty = self.max_capacity.saturating_sub(occupied); // If occupied beyond capacity, just say no empty space
//         if self.sequence.total() <= self.low_capacity {
//             TruckStockState::Empty { occupied, empty }
//         } else if self.sequence.total() >= self.max_capacity {
//             TruckStockState::Full { occupied, empty }
//         } else {
//             TruckStockState::Normal { occupied, empty }
//         }
//     }
//     fn get_previous_state(&mut self) -> &Option<Self::StockState> {
//         &self.prev_state
//     }
//     fn set_previous_state(&mut self) {
//         self.prev_state = Some(self.get_state());
//     }
//     fn get_resource(&self) -> &SeqDeque {
//         &self.sequence
//     }
//     fn add_impl<'a>(&'a mut self, payload: &'a (Option<TruckAndIronOre>, NotificationMetadata), cx: &'a mut Context<Self>) -> impl Future<Output = ()> + 'a {
//         async move {
//             self.prev_state = Some(self.get_state());
//             match payload.0 {
//                 Some(ref item) => {
//                     self.sequence.deque.push_back(item.clone());
//                 }
//                 None => {}
//             }
//         }
//     }
//     fn remove_impl<'a>(&'a mut self, payload: &'a ((), NotificationMetadata), cx: &'a mut Context<Self>) -> impl Future<Output = Option<T>> + 'a {
//         async move {
//             self.prev_state = Some(self.get_state());
//             self.sequence.deque.pop_front()
//         }
//     }

//     fn emit_change<'a>(&'a mut self, payload: NotificationMetadata, cx: &'a mut nexosim::model::Context<Self>) -> impl Future<Output = ()> + Send + 'a {
//         async move {
//             self.state_emitter.send(payload).await;
//             self.log(cx.time(), "Emit Change".to_string()).await;
//         }
//     }

//     fn log(&mut self, time: MonotonicTime, log_type: String) -> impl Future<Output = ()> + Send {
//         async move {
//             let log = TruckStockLog {
//                 time: time.to_chrono_date_time(0).unwrap().to_string(),
//                 event_id: self.next_event_id,
//                 element_name: self.element_name.clone(),
//                 element_type: self.element_type.clone(),
//                 log_type,
//                 state: self.get_state(),
//                 sequence: self.sequence.clone(),
//             };
//             self.log_emitter.send(log).await;
//             self.next_event_id += 1;
//         }
//     }
// }

// impl Model for TruckStock {}

// impl TruckStock {
//     pub fn new() -> Self {
//         TruckStock::default()
//     }
//     pub fn with_name(mut self, name: String) -> Self {
//         self.element_name = name;
//         self
//     }
//     pub fn with_type(mut self, type_: String) -> Self {
//         self.element_type = type_;
//         self
//     }

//     pub fn with_initial_contents(mut self, contents: Vec<TruckAndIronOre>) -> Self {
//         self.sequence.deque = contents.into_iter().collect();
//         self
//     }

//     pub fn with_low_capacity(mut self, low_capacity: u32) -> Self {
//         self.low_capacity = low_capacity;
//         self
//     }

//     pub fn with_max_capacity(mut self, max_capacity: u32) -> Self {
//         self.max_capacity = max_capacity;
//         self
//     }
// }






define_model_enums! {
    pub enum ComponentModel {
        IronOreStock(VectorStock<IronOre>, Mailbox<VectorStock<IronOre>>),
        DiscreteStockTruck(DiscreteStock<Truck>, Mailbox<DiscreteStock<Truck>>),
        LoadingProcess(LoadingProcess, Mailbox<LoadingProcess>),
        DiscreteParallelProcessTruck(DiscreteParallelProcess<Option<Truck>, (), Option<Truck>>, Mailbox<DiscreteParallelProcess<Option<Truck>, (), Option<Truck>>>),
    }
    pub enum ComponentModelAddress {
        IronOreStock(Address<VectorStock<IronOre>>),
        DiscreteStockTruck(Address<DiscreteStock<Truck>>),
        LoadingProcess(Address<LoadingProcess>),
        DiscreteParallelProcessTruck(Address<DiscreteParallelProcess<Option<Truck>, (), Option<Truck>>>),
    }
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
                // TODO: Implement properly
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
            Truck { ore: None, truck_id: "Truck_02".into() },
            Truck { ore: None, truck_id: "Truck_03".into() },
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

    let mut ready_to_dump = ComponentModel::DiscreteStockTruck(DiscreteStock::new()
        .with_name("ready_to_dump".into())
        .with_type("TruckStock".into())
        .with_initial_contents(Vec::new())
        .with_low_capacity(0)
        .with_max_capacity(10),
        Mailbox::new(),
    );

    // Connect components

    connect_components!(&mut source_sp, &mut loading_process).unwrap();
    connect_components!(&mut trucks_ready_to_load, &mut loading_process).unwrap();
    connect_components!(&mut loading_process, &mut trucks_loaded).unwrap();
    connect_components!(&mut trucks_loaded, &mut loaded_truck_movements).unwrap();
    connect_components!(&mut loaded_truck_movements, &mut ready_to_dump).unwrap();

    let mut ore_stock_logger = ComponentLogger::IronOreStockLogger(VectorStockLogger::new("OreStockLogger".into()));
    let mut truck_stock_logger = ComponentLogger::TruckStockLogger(DiscreteStockLogger::new("TruckStockLogger".into()));
    let mut truck_process_logger = ComponentLogger::TruckingProcessLogger(TruckingProcessLogger::new("TruckProcessLogger".into()));

    connect_logger!(&mut ore_stock_logger, &mut source_sp).unwrap();
    connect_logger!(&mut truck_stock_logger, &mut trucks_ready_to_load).unwrap();
    connect_logger!(&mut truck_stock_logger, &mut trucks_loaded).unwrap();
    connect_logger!(&mut truck_stock_logger, &mut ready_to_dump).unwrap();
    connect_logger!(&mut truck_process_logger, &mut loading_process).unwrap();
    connect_logger!(&mut truck_process_logger, &mut loaded_truck_movements).unwrap();

    let mut sim_builder = SimInit::new();
    sim_builder = register_component!(sim_builder, source_sp);
    sim_builder = register_component!(sim_builder, trucks_ready_to_load);
    sim_builder = register_component!(sim_builder, loading_process);
    sim_builder = register_component!(sim_builder, trucks_loaded);
    sim_builder = register_component!(sim_builder, loaded_truck_movements);
    sim_builder = register_component!(sim_builder, ready_to_dump);

    let start_time = MonotonicTime::try_from_date_time(2025, 5, 1, 0, 0, 0, 0).unwrap();
    let mut simu = sim_builder.init(start_time.clone()).unwrap().0;

    loading_process_addr.initialise(&mut simu).unwrap();
    loaded_truck_movements_addr.initialise(&mut simu).unwrap();

    simu.step_until(start_time + Duration::from_secs(600)).unwrap();

    let output_dir = "outputs/diegos_trucking";
    create_dir_all(output_dir).unwrap();

    // ore_stock_logger.write_csv(output_dir.into()).unwrap();
    // truck_stock_logger.write_csv(output_dir.into()).unwrap();
    // truck_process_logger.write_csv(output_dir.into()).unwrap();

}
