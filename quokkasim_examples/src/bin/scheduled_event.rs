#[allow(clippy::manual_async_fn)]

use std::{error::Error, fs::create_dir_all, time::Duration};

use nexosim::time::MonotonicTime;
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
    // Declarations

    let mut stock_1 = ComponentModel::VectorStockF64(
        VectorStock::new()
            .with_name("Stock 1".into())
            .with_low_capacity(50.)
            .with_max_capacity(101.)
            .with_initial_vector(100.),
        Mailbox::new()
    );
    let stock_1_addr = stock_1.get_address();
    let mut stock_2 = ComponentModel::VectorStockF64(
        VectorStock::new()
            .with_name("Stock 2".into())
            .with_low_capacity(50.)
            .with_max_capacity(101.)
            .with_initial_vector(0.),
        Mailbox::new()
    );
    let mut process = ComponentModel::VectorProcessF64(
        VectorProcess::new()
            .with_name("Process".into())
            .with_process_quantity_distr(Distribution::Constant(1.))
            .with_process_time_distr(Distribution::Constant(1.)),
        Mailbox::new()
    );

    let mut process_logger = ComponentLogger::VectorProcessLoggerF64(VectorProcessLogger::new("ProcessLogger".into()));
    let mut stock_logger = ComponentLogger::VectorStockLoggerF64(VectorStockLogger::new("StockLogger".into()));

    // Connect components
    connect_components!(&mut stock_1, &mut process).unwrap();
    connect_components!(&mut process, &mut stock_2).unwrap();

    // Connect loggers
    connect_logger!(&mut process_logger, &mut process).unwrap();
    connect_logger!(&mut stock_logger, &mut stock_1).unwrap();
    connect_logger!(&mut stock_logger, &mut stock_2).unwrap();

    // Create simulation
    let mut sim_builder = SimInit::new();

    sim_builder = register_component!(sim_builder, stock_1);
    sim_builder = register_component!(sim_builder, process);
    sim_builder = register_component!(sim_builder, stock_2);

    let start_time = MonotonicTime::try_from_date_time(2025, 1, 1, 0, 0, 0, 0).unwrap();
    let (mut simu, mut scheduler) = sim_builder.init(start_time.clone()).unwrap();

    let capacity_change = ScheduledEventConfig::SetLowCapacity(10.);

    let mut df = DistributionFactory {
        base_seed: 1234,
        next_seed: 0,
    };

    let event_time = start_time.clone() + Duration::from_secs(60);
    
    create_scheduled_event!(&mut scheduler, &event_time, &capacity_change, &stock_1_addr, &mut df).unwrap();
    simu.step_until(start_time + Duration::from_secs(120)).unwrap();

    let output_dir = "outputs/scheduled_event";
    create_dir_all(output_dir).unwrap();
    stock_logger.write_csv(&output_dir).unwrap();
    process_logger.write_csv(&output_dir).unwrap();
}