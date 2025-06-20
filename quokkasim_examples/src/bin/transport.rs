#![allow(clippy::manual_async_fn)]

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
    // Declarations

    // let mut source = ComponentModel::Vector3Source(
    //     VectorSource::new()
    //         .with_name("Source".into())
    //         .with_process_quantity_distr(Distribution::Constant(1.))
    //         .with_process_time_distr(Distribution::Constant(1.))
    //         .with_source_vector([1., 4., 5.].into()),
    //     Mailbox::new()
    // );

    let mut df = DistributionFactory::new(1234);

    let mut stock_1 = ComponentModel::Vector3Stock(
        VectorStock::new()
            .with_name("Stock 1".into())
            .with_low_capacity(10.)
            .with_max_capacity(100.)
            .with_initial_resource([ 99., 0., 0. ].into()),
        Mailbox::new()
    );
    let mut process = ComponentModel::Vector3Process(
        VectorProcess::new()
            .with_name("Process".into())
            .with_process_quantity_distr(Distribution::Constant(1.))
            .with_process_time_distr(Distribution::Constant(1.))
            .with_delay_mode(DelayModeChange::Add(DelayMode {
                name: "ShortDelay".into(), 
                until_delay_distr: df.create(DistributionConfig::Uniform { min: 5., max: 10. }).unwrap(),
                until_fix_distr: Distribution::Constant(1.),
            }))
            .with_delay_mode(DelayModeChange::Add(DelayMode {
                name: "LongDelay".into(), 
                until_delay_distr: df.create(DistributionConfig::Uniform { min: 40., max: 60. }).unwrap(),
                until_fix_distr: Distribution::Constant(5.),
            })),
        Mailbox::new()
    );
    let mut stock_2 = ComponentModel::Vector3Stock(
        VectorStock::new()
            .with_name("Stock 2".into())
            .with_low_capacity(10.)
            .with_max_capacity(100.)
            .with_initial_resource([0., 0., 0.].into()),
        Mailbox::new()
    );

    // let mut sink = ComponentModel::Vector3Sink(
    //     VectorSink::new()
    //         .with_name("Sink".into())
    //         .with_process_quantity_distr(Distribution::Constant(1.))
    //         .with_process_time_distr(Distribution::Constant(2.)),
    //     Mailbox::new()
    // );

    let mut process_logger = ComponentLogger::Vector3ProcessLogger(VectorProcessLogger::new("ProcessLogger".into()));
    let mut stock_logger = ComponentLogger::Vector3StockLogger(VectorStockLogger::new("StockLogger".into()));

    // Connect components
    // connect_components!(&mut source, &mut stock_1).unwrap();
    connect_components!(&mut stock_1, &mut process).unwrap();
    connect_components!(&mut process, &mut stock_2).unwrap();
    // connect_components!(&mut stock_2, &mut sink).unwrap();

    // Connect loggers
    // connect_logger!(&mut process_logger, &mut source).unwrap();
    connect_logger!(&mut process_logger, &mut process).unwrap();
    // connect_logger!(&mut process_logger, &mut sink).unwrap();
    connect_logger!(&mut stock_logger, &mut stock_1).unwrap();
    connect_logger!(&mut stock_logger, &mut stock_2).unwrap();

    // Create simulation
    let mut sim_builder = SimInit::new();

    sim_builder = register_component!(sim_builder, stock_1);
    sim_builder = register_component!(sim_builder, stock_2);
    // sim_builder = register_component!(sim_builder, source);
    sim_builder = register_component!(sim_builder, process);
    // sim_builder = register_component!(sim_builder, sink);

    let start_time = MonotonicTime::try_from_date_time(2025, 1, 1, 0, 0, 0, 0).unwrap();
    let (mut simu, scheduler) = sim_builder.init(start_time).unwrap();

    simu.step_until(start_time + Duration::from_secs(120)).unwrap();

    let output_dir = "outputs/transport";
    create_dir_all(output_dir).unwrap();
    stock_logger.write_csv(output_dir).unwrap();
    process_logger.write_csv(output_dir).unwrap();
}