#![allow(clippy::manual_async_fn)]

use std::{error::Error, fs::create_dir_all, time::Duration};
use quokkasim::{define_model_enums, prelude::*};


define_model_enums! {
    pub enum ComponentModel {}
    pub enum ComponentModelAddress {}
    pub enum ComponentLogger {}
    pub enum ScheduledEvent {}
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

    let mut df = DistributionFactory {
        base_seed: 1234,
        next_seed: 0,
    };

    let mut source = ComponentModel::StringSource(DiscreteSource::new().with_name("Source").with_process_time_distr(Distribution::Constant(3.)), Mailbox::new());

    let mut queue_1 = ComponentModel::StringStock(DiscreteStock::new()
        .with_name("Queue1")
        .with_low_capacity(0)
        .with_max_capacity(10)
        .with_initial_resource(ItemDeque::default()),
        Mailbox::new()
    );

    let mut process_1 = ComponentModel::StringProcess(DiscreteProcess::new()
        .with_name("Process1")
        .with_process_time_distr(df.create(DistributionConfig::Triangular { min: 1., max: 10., mode: 6. }).unwrap()),
        Mailbox::new()
    );

    let mut queue_2 = ComponentModel::StringStock(DiscreteStock::new()
        .with_name("Queue2")
        .with_low_capacity(0)
        .with_max_capacity(10)
        .with_initial_resource(ItemDeque::default()),
        Mailbox::new()
    );

    let mut process_par = ComponentModel::StringProcess(DiscreteProcess::new()
        .with_name("Process2")
        .with_process_time_distr(df.create(DistributionConfig::Triangular { min: 1., max: 10., mode: 6. }).unwrap()),
        Mailbox::new()
    );

    let mut queue_3 = ComponentModel::StringStock(DiscreteStock::new()
        .with_name("Queue3")
        .with_low_capacity(0)
        .with_max_capacity(10)
        .with_initial_resource(ItemDeque::default()),
        Mailbox::new()
    );

    let mut sink = ComponentModel::StringSink(DiscreteSink::new()
        .with_name("Sink")
        .with_process_time_distr(Distribution::Constant(1.)),
        Mailbox::new()
    );

    connect_components!(&mut source, &mut queue_1).unwrap();
    connect_components!(&mut queue_1, &mut process_1).unwrap();
    connect_components!(&mut process_1, &mut queue_2).unwrap();
    connect_components!(&mut queue_2, &mut process_par).unwrap();
    connect_components!(&mut process_par, &mut queue_3).unwrap();
    connect_components!(&mut queue_3, &mut sink).unwrap();

    let mut queue_logger = ComponentLogger::StringStockLogger(DiscreteStockLogger::new("QueueLogger".into()));
    let mut process_logger = ComponentLogger::StringProcessLogger(DiscreteProcessLogger::new("ProcessLogger".into()));

    connect_logger!(&mut queue_logger, &mut queue_1).unwrap();
    connect_logger!(&mut queue_logger, &mut queue_2).unwrap();
    connect_logger!(&mut queue_logger, &mut queue_3).unwrap();
    connect_logger!(&mut process_logger, &mut source).unwrap();
    connect_logger!(&mut process_logger, &mut process_1).unwrap();
    connect_logger!(&mut process_logger, &mut process_par).unwrap();
    connect_logger!(&mut process_logger, &mut sink).unwrap();

    let mut sim_builder = SimInit::new();
    sim_builder = register_component!(sim_builder, source);
    sim_builder = register_component!(sim_builder, queue_1);
    sim_builder = register_component!(sim_builder, process_1);
    sim_builder = register_component!(sim_builder, queue_2);
    sim_builder = register_component!(sim_builder, process_par);
    sim_builder = register_component!(sim_builder, queue_3);
    sim_builder = register_component!(sim_builder, sink);

    let mut simu = sim_builder.init(MonotonicTime::EPOCH).unwrap().0;

    simu.step_until(MonotonicTime::EPOCH + Duration::from_secs(200)).unwrap();

    let output_dir = "outputs/discrete_queue";
    create_dir_all(output_dir).unwrap();
    queue_logger.write_csv(output_dir).unwrap();
    process_logger.write_csv(output_dir).unwrap();
}