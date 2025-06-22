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

    let mut stock1 = ComponentModel::F64Stock(
        VectorStock::new()
            .with_name("Stock1")
            .with_code("S1")
            .with_initial_resource(100.)
            .with_low_capacity(10.)
            .with_max_capacity(1000.),
        Mailbox::new()
    );

    // let mut splitter = ComponentModel::F64Splitter2(                
    //     VectorSplitter::new()
    //         .with_name("Splitter")
    //         .with_code("P1")
    //         .with_process_quantity_distr(Distribution::Constant(10.))
    //         .with_process_time_distr(Distribution::Constant(10.))
    //         .with_delay_mode(DelayModeChange::Add(DelayMode { name: "SplitterDelay".into(), until_delay_distr: Distribution::Constant(7.), until_fix_distr: Distribution::Constant(2.) })),
    //     Mailbox::new()
    // );
    let mut process = ComponentModel::F64Process(
        VectorProcess::new()
            .with_name("Process")
            .with_code("P1")
            .with_process_quantity_distr(Distribution::Constant(10.))
            .with_process_time_distr(Distribution::Constant(10.))
            .with_delay_mode(DelayModeChange::Add(DelayMode { name: "ProcessDelay".into(), until_delay_distr: Distribution::Constant(7.), until_fix_distr: Distribution::Constant(2.) })),
        Mailbox::new()
    );

    let mut stock2 = ComponentModel::F64Stock(
        VectorStock::new()
            .with_name("Stock2")
            .with_code("S2")
            .with_low_capacity(10.)
            .with_max_capacity(1000.),
        Mailbox::new()
    );

    let mut env = ComponentModel::BasicEnvironment(
        BasicEnvironment::new()
            .with_name("Environment")
            .with_code("E1"),
        Mailbox::new()
    );
    let env_addr = env.get_address();

    let mut df = DistributionFactory::new(12);

    // let mut stock3 = ComponentModel::F64Stock(
    //     VectorStock::new()
    //         .with_name("Stock3")
    //         .with_code("S3")
    //         .with_low_capacity(10.)
    //         .with_max_capacity(1000.),
    //     Mailbox::new()
    // );

    connect_components!(&mut stock1, &mut process).unwrap();
    connect_components!(&mut process, &mut stock2, 0).unwrap();
    connect_components!(&mut env, &mut process).unwrap();
    // connect_components!(&mut splitter, &mut stock3, 1).unwrap();

    let mut stock_logger = ComponentLogger::VectorStockLoggerF64(VectorStockLogger::new("StockLogger".into()));
    let mut process_logger = ComponentLogger::VectorProcessLoggerF64(VectorProcessLogger::new("ProcessLogger".into()));
    let mut env_logger = ComponentLogger::BasicEnvironmentLogger(BasicEnvironmentLogger::new("EnvLogger".into()));

    connect_logger!(&mut stock_logger, &mut stock1).unwrap();
    connect_logger!(&mut stock_logger, &mut stock2).unwrap();
    // connect_logger!(&mut stock_logger, &mut stock3).unwrap();
    connect_logger!(&mut process_logger, &mut process).unwrap();
    connect_logger!(&mut env_logger, &mut env).unwrap();
    
    let mut sim_builder = SimInit::new();
    sim_builder = register_component!(sim_builder, stock1);
    sim_builder = register_component!(sim_builder, stock2);
    // sim_builder = register_component!(sim_builder, stock3);
    sim_builder = register_component!(sim_builder, process);
    sim_builder = register_component!(sim_builder, env);


    let (mut simu, mut sched) = sim_builder.init(MonotonicTime::EPOCH).unwrap();

    let event_time = MonotonicTime::EPOCH + Duration::from_secs(4);
    let sched_event = ScheduledEvent::SetEnvironmentState(BasicEnvironmentState::Stopped);
    create_scheduled_event!(&mut sched, &event_time, &sched_event, &env_addr, &mut df);

    let event_time = MonotonicTime::EPOCH + Duration::from_secs(24);
    let sched_event = ScheduledEvent::SetEnvironmentState(BasicEnvironmentState::Normal);
    create_scheduled_event!(&mut sched, &event_time, &sched_event, &env_addr, &mut df);

    simu.step_until(MonotonicTime::EPOCH + Duration::from_secs(40)).unwrap();

    let output_dir = "outputs/delay_testing";
    create_dir_all(output_dir).unwrap();
    stock_logger.write_csv(output_dir).unwrap();
    process_logger.write_csv(output_dir).unwrap();
    env_logger.write_csv(output_dir).unwrap();
}