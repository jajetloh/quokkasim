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
    let mut df = DistributionFactory::new(3214);

    let mut car_arrivals = ComponentModel::StringSource(
        DiscreteSource::new()
            .with_name("Car Arrivals")
            .with_code("CA")
            .with_item_factory(StringItemFactory::default())
            .with_process_quantity_distr(Distribution::Constant(1.))
            .with_process_time_distr(df.create(DistributionConfig::Uniform { min: 300., max: 1200. }).unwrap()),
        Mailbox::new()
    );
    let mut ready_to_service = ComponentModel::StringStock(
        DiscreteStock::new()
            .with_name("Ready to Service")
            .with_code("RTS")
            .with_low_capacity(0)
            .with_max_capacity(100),
        Mailbox::new()
    );
    let mut car_hoists = (0..3).into_iter().map(|i| {
        ComponentModel::StringProcess(
            DiscreteProcess::new()
                .with_name(&format!("Car Hoist {}", i + 1))
                .with_code(&format!("CH{}", i + 1))
                .with_process_quantity_distr(Distribution::Constant(1.))
                .with_process_time_distr(df.create(DistributionConfig::Uniform { min: 600., max: 1800. }).unwrap()),
            Mailbox::new()
        )
    }).collect::<Vec<_>>();
    let mut ready_to_depart = ComponentModel::StringStock(
        DiscreteStock::new()
            .with_name("Ready to Depart")
            .with_code("RTD")
            .with_low_capacity(0)
            .with_max_capacity(100),
        Mailbox::new()
    );
    let mut car_departures = ComponentModel::StringSink(
        DiscreteSink::new()
            .with_name("Car Departures")
            .with_code("CD"),
        Mailbox::new()
    );

    connect_components!(&mut car_arrivals, &mut ready_to_service).unwrap();
    car_hoists.iter_mut().for_each(|mut hoist| {
        connect_components!(&mut ready_to_service, &mut hoist).unwrap();
        connect_components!(&mut hoist, &mut ready_to_depart).unwrap();
    });
    connect_components!(&mut ready_to_depart, &mut car_departures).unwrap();

    let mut process_logger = ComponentLogger::StringProcessLogger(DiscreteProcessLogger::new("CarProcessLogger"));
    let mut stock_logger = ComponentLogger::StringStockLogger(DiscreteStockLogger::new("CarStockLogger"));

    car_hoists.iter_mut().for_each(|mut hoist| {
        connect_logger!(&mut process_logger, &mut hoist).unwrap();
    });
    connect_logger!(&mut process_logger, &mut car_arrivals).unwrap();
    connect_logger!(&mut process_logger, &mut car_departures).unwrap();
    connect_logger!(&mut stock_logger, &mut ready_to_service).unwrap();
    connect_logger!(&mut stock_logger, &mut ready_to_depart).unwrap();

    let mut sim_init = SimInit::new();
    sim_init = register_component!(sim_init, car_arrivals);
    sim_init = register_component!(sim_init, ready_to_service);
    for hoist in car_hoists {
        sim_init = register_component!(sim_init, hoist);
    }
    sim_init = register_component!(sim_init, ready_to_depart);
    sim_init = register_component!(sim_init, car_departures);

    let start_time = MonotonicTime::try_from_date_time(2025, 6, 1, 8, 0, 0, 0).unwrap();
    let (mut sim, mut sched) = sim_init.init(start_time.clone()).unwrap();
    
    sim.step_until(start_time + Duration::from_secs(60 * 60 * 24 * 30)).unwrap();

    let output_dir = "outputs/car_workshop";
    create_dir_all(output_dir).unwrap();
    process_logger.write_csv(output_dir).unwrap();
    stock_logger.write_csv(output_dir).unwrap();
}