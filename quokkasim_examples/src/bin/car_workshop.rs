use std::{error::Error, fs::create_dir_all, time::Duration};
use quokkasim::{define_model_enums, prelude::*};

define_model_enums! {
    pub enum ComponentModel {}
    pub enum ComponentModelAddress {}
    pub enum ComponentLogger {}
    pub enum ScheduledEventConfig {}
}

impl CustomComponentConnection for ComponentModel {
    fn connect_components(a: &mut Self, b: &mut Self, n: Option<usize>) -> Result<(), Box<dyn Error>> {
        match (a, b) {
            (a, b) => Err(format!("No component connection defined from {} to {} (n={:?})", a, b, n).into()),
        }
    }
}

impl CustomLoggerConnection for ComponentLogger { 
    type ComponentType = ComponentModel;
    fn connect_logger(a: &mut Self, b: &mut Self::ComponentType, n: Option<usize>) -> Result<(), Box<dyn Error>> {
        match (a, b, n) {
            (a, b, _) => Err(format!("No logger connection defined from {} to {} (n={:?})", a, b, n).into()),
        }
    }
}


fn main() {

    let mut arrivals = ComponentModel::StringSource(
        DiscreteSource::new()
            .with_name("Arrivals")
            .with_code("A")
            .with_item_factory(StringItemFactory { prefix: "Car".into(), next_index: 0, num_digits: 4 })
            .with_process_time_distr(Distribution::Constant(900.)),
        Mailbox::new()
    );

    let mut ready_to_service = ComponentModel::StringStock(
        DiscreteStock::new()
            .with_name("Ready to Service")
            .with_code("Q1")
            .with_low_capacity(0)
            .with_max_capacity(3),
        Mailbox::new()
    );

    let mut car_hoists = (0..3).into_iter().map(|i| {
        ComponentModel::StringProcess(
            DiscreteProcess::new()
                .with_name(&format!("Car Hoist {}", i))
                .with_code(&format!("P{}", i))
                .with_process_time_distr(Distribution::Constant(1800.)),
            Mailbox::new()
        )
    }).collect::<Vec<_>>();

    let mut ready_to_depart = ComponentModel::StringStock(
        DiscreteStock::new()
            .with_name("Ready to Depart")
            .with_code("Q2")
            .with_low_capacity(0)
            .with_max_capacity(3),
        Mailbox::new()
    );

    let mut departures = ComponentModel::StringSink(
        DiscreteSink::new()
            .with_name("Departures")
            .with_code("D")
            .with_process_time_distr(Distribution::Constant(1.)),
        Mailbox::new()
    );

    connect_components!(&mut arrivals, &mut ready_to_service).unwrap();

    for mut hoist in car_hoists.iter_mut() {
        connect_components!(&mut ready_to_service, &mut hoist).unwrap();
        connect_components!(&mut hoist, &mut ready_to_depart).unwrap();
    }
    connect_components!(&mut ready_to_depart, &mut departures).unwrap();

    let mut process_logger = ComponentLogger::StringProcessLogger(DiscreteProcessLogger::new("ProcessLogger"));
    let mut stock_logger = ComponentLogger::StringStockLogger(DiscreteStockLogger::new("StockLogger"));

    connect_logger!(&mut process_logger, &mut arrivals).unwrap();
    for mut car_hoist in car_hoists.iter_mut() {
        connect_logger!(&mut process_logger, &mut car_hoist).unwrap();
    }
    connect_logger!(&mut process_logger, &mut departures).unwrap();
    connect_logger!(&mut stock_logger, &mut ready_to_service).unwrap();
    connect_logger!(&mut stock_logger, &mut ready_to_depart).unwrap();

    let mut sim_init = SimInit::new();
    sim_init = register_component!(sim_init, arrivals);
    sim_init = register_component!(sim_init, ready_to_service);
    for hoist in car_hoists {
        sim_init = register_component!(sim_init, hoist);
    }
    sim_init = register_component!(sim_init, ready_to_depart);
    sim_init = register_component!(sim_init, departures);

    let start_time = MonotonicTime::try_from_date_time(2025, 7, 1, 8, 0, 0, 0).unwrap();
    let (mut sim, mut scheduler) = sim_init.init(start_time).unwrap();
    sim.step_until(start_time + Duration::from_secs(3600 * 9)).unwrap();

    let output_dir = "output/car_workshop";
    create_dir_all(output_dir).unwrap();
    process_logger.write_csv(output_dir).unwrap();
    stock_logger.write_csv(output_dir).unwrap();
}