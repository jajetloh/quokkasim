#![allow(clippy::manual_async_fn)]

use std::{error::Error, fs::create_dir_all, ops::{Deref, DerefMut}, time::Duration};

use nexosim::time::MonotonicTime;
use quokkasim::{define_model_enums, prelude::*};
use serde::Serialize;


struct CarGenerator {
    fuel_level_distr: Distribution,
    next_id: usize,
}

impl Default for CarGenerator {
    fn default() -> Self {
        CarGenerator {
            fuel_level_distr: Distribution::Constant(1.0),
            next_id: 0,
        } 
    }
}

impl ItemFactory<Car> for CarGenerator {
    fn create_item(&mut self) -> Car {
        let car = Car {
            id: self.next_id,
            tank_size_liters: 50.0,
            tank_level_liters: self.fuel_level_distr.sample(),
        };
        self.next_id += 1;
        car
    }
}

struct FuelStation(DiscreteParallelProcess<(), Option<Car>, Car, Car>);
impl Deref for FuelStation { type Target = DiscreteParallelProcess<(), Option<Car>, Car, Car>; fn deref(&self) -> &Self::Target { &self.0 } }
impl DerefMut for FuelStation { fn deref_mut(&mut self) -> &mut Self::Target { &mut self.0 } }

impl Model for FuelStation {
    fn init(mut self, cx: &mut Context<Self>) -> impl Future<Output = InitializedModel<Self>> + Send {
        async move {
            self.0.update_state(EventId::from_init(), &mut Self::as_inner_context(cx)).await;
            self.into()
        }
    }
}

impl From<DiscreteParallelProcess<(), Option<Car>, Car, Car>> for FuelStation {
    fn from(process: DiscreteParallelProcess<(), Option<Car>, Car, Car>) -> Self {
        FuelStation(process)
    }
}

impl FuelStation {
    fn as_inner_context(cx: &mut Context<Self>) -> &mut Context<DiscreteParallelProcess<(), Option<Car>, Car, Car>> {
        unsafe { &mut *(cx as *mut _ as *mut Context<DiscreteParallelProcess<(), Option<Car>, Car, Car>>) }
    }

    fn update_state(&mut self, event_id: EventId, cx: &mut Context<Self>) -> impl Future<Output = ()> + Send {
        async move {
            self.0.update_state(event_id, FuelStation::as_inner_context(cx)).await;
        }
    }

    fn new() -> Self {
        FuelStation(DiscreteParallelProcess::new())
    }
}

define_model_enums! {
    pub enum ComponentModel {
        CarArrivals(DiscreteSource<Car, Car, CarGenerator>, Mailbox<DiscreteSource<Car, Car, CarGenerator>>),
        CarQueue(DiscreteStock<Car>, Mailbox<DiscreteStock<Car>>),
        FuelStation(FuelStation, Mailbox<FuelStation>),
        CarDepartures(DiscreteSink<(), Option<Car>, Car>, Mailbox<DiscreteSink<(), Option<Car>, Car>>),
    }
    pub enum ComponentModelAddress {}
    pub enum ComponentLogger {
        CarProcessLogger(DiscreteProcessLogger<Car>),
        CarStockLogger(DiscreteStockLogger<Car>),
    }
    pub enum ScheduledEvent {}
}


impl CustomComponentConnection for ComponentModel {
    fn connect_components(a: &mut Self, b: &mut Self, n: Option<usize>) -> Result<(), Box<dyn Error>> {
        match (a, b) {
            (ComponentModel::CarArrivals(a, a_mbox), ComponentModel::CarQueue(b, b_mbox)) => {
                a.req_downstream.connect(DiscreteStock::get_state_async, b_mbox.address());
                a.push_downstream.connect(DiscreteStock::add, b_mbox.address());
                b.state_emitter.connect(DiscreteSource::update_state, a_mbox.address());
                Ok(())
            },
            (ComponentModel::CarQueue(a, a_mbox), ComponentModel::FuelStation(b, b_mbox)) => {
                b.req_upstream.connect(DiscreteStock::get_state_async, a_mbox.address());
                b.withdraw_upstream.connect(DiscreteStock::remove, a_mbox.address());
                a.state_emitter.connect(FuelStation::update_state, b_mbox.address());
                Ok(())
            },
            (ComponentModel::FuelStation(a, a_mbox), ComponentModel::CarQueue(b, b_mbox)) => {
                a.req_downstream.connect(DiscreteStock::get_state_async, b_mbox.address());
                a.push_downstream.connect(DiscreteStock::add, b_mbox.address());
                b.state_emitter.connect(FuelStation::update_state, a_mbox.address());
                Ok(())
            },
            (ComponentModel::CarQueue(a, a_mbox), ComponentModel::CarDepartures(b, b_mbox)) => {
                b.req_upstream.connect(DiscreteStock::get_state_async, a_mbox.address());
                b.withdraw_upstream.connect(DiscreteStock::remove, a_mbox.address());
                a.state_emitter.connect(DiscreteSink::update_state, b_mbox.address());
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
            (ComponentLogger::CarProcessLogger(a), ComponentModel::CarArrivals(b, _), _) => {
                b.log_emitter.connect_sink(&a.buffer);
                Ok(())
            },
            (ComponentLogger::CarProcessLogger(a), ComponentModel::FuelStation(b, _), _) => {
                b.log_emitter.connect_sink(&a.buffer);
                Ok(())
            },
            (ComponentLogger::CarProcessLogger(a), ComponentModel::CarDepartures(b, _), _) => {
                b.log_emitter.connect_sink(&a.buffer);
                Ok(())
            },
            (ComponentLogger::CarStockLogger(a), ComponentModel::CarQueue(b, _), _) => {
                b.log_emitter.connect_sink(&a.buffer);
                Ok(())
            },
            (a, b, _) => Err(format!("No logger connection defined from {} to {} (n={:?})", a, b, n).into()),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize)]
struct Car {
    id: usize,
    tank_size_liters: f64,
    tank_level_liters: f64,
}



fn main() {
    // Declarations

    let mut df = DistributionFactory {
        base_seed: 1234,
        next_seed: 0,
    };

    let car_arrival_generator =  CarGenerator {
        fuel_level_distr: df.create(DistributionConfig::Uniform { min: 5., max: 25. }).unwrap(),
        next_id: 0,
    };
    let mut car_arrivals = ComponentModel::CarArrivals(
        DiscreteSource::new()
            .with_name("Car Arrivals")
            .with_code("IN") 
            .with_process_time_distr(Distribution::Constant(1.0))
            .with_item_factory(car_arrival_generator),
        Mailbox::new()
    );

    let mut cars_waiting = ComponentModel::CarQueue(
        DiscreteStock::new()
            .with_name("Cars Waiting")
            .with_code("Q1") 
            .with_low_capacity(0)
            .with_max_capacity(10),
        Mailbox::new()
    );

    let mut fs = FuelStation::new();
    fs.element_name = "Fuel Station".into();
    fs.element_code = "FS".into();
    fs.process_time_distr = Distribution::Constant(5.0);
    let mut fuel_station = ComponentModel::FuelStation(
        fs,
        Mailbox::new()
    );

    let mut cars_leaving = ComponentModel::CarQueue(
        DiscreteStock::new()
            .with_name("Cars Waiting")
            .with_code("Q2") 
            .with_low_capacity(0)
            .with_max_capacity(10),
        Mailbox::new()
    );

    let mut cars_departing = ComponentModel::CarDepartures(
        DiscreteSink::new()
            .with_name("Car Departures")
            .with_code("OUT") 
            .with_process_time_distr(Distribution::Constant(1.0)),
        Mailbox::new()
    );

    connect_components!(&mut car_arrivals, &mut cars_waiting).unwrap();
    connect_components!(&mut cars_waiting, &mut fuel_station).unwrap();
    connect_components!(&mut fuel_station, &mut cars_leaving).unwrap();
    connect_components!(&mut cars_leaving, &mut cars_departing).unwrap();

    let mut process_logger = ComponentLogger::CarProcessLogger(DiscreteProcessLogger::new("ProcessLogger".into()));
    let mut stock_logger = ComponentLogger::CarStockLogger(DiscreteStockLogger::new("StockLogger".into()));

    connect_logger!(&mut process_logger, &mut car_arrivals).unwrap();
    connect_logger!(&mut process_logger, &mut fuel_station).unwrap();
    connect_logger!(&mut process_logger, &mut cars_departing).unwrap();
    connect_logger!(&mut stock_logger, &mut cars_waiting).unwrap();
    connect_logger!(&mut stock_logger, &mut cars_leaving).unwrap();

    let mut sim_init = SimInit::new();
    sim_init = register_component!(sim_init, car_arrivals);
    sim_init = register_component!(sim_init, cars_waiting);
    sim_init = register_component!(sim_init, fuel_station);
    sim_init = register_component!(sim_init, cars_leaving);
    sim_init = register_component!(sim_init, cars_departing);

    let start_time = MonotonicTime::EPOCH;
    let duration = Duration::from_secs(60 * 60); // 1 hour

    let mut simu = sim_init.init(start_time).unwrap().0;
    simu.step_until(start_time + duration).unwrap();

    let output_dir = "outputs/gas_station_refueling";
    create_dir_all(output_dir).unwrap();
    process_logger.write_csv(output_dir).unwrap();
    stock_logger.write_csv(output_dir).unwrap();
}
