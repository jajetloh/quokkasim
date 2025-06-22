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
            .with_initial_resource(1000.)
            .with_low_capacity(10.)
            .with_max_capacity(1000.),
        Mailbox::new()
    );

    let mut splitter = ComponentModel::F64Splitter2(                
        VectorSplitter::new()
            .with_name("Splitter")
            .with_code("P1")
            .with_process_quantity_distr(Distribution::Constant(10.))
            .with_process_time_distr(Distribution::Constant(10.)),
        Mailbox::new()
    );

    let mut stock2 = ComponentModel::F64Stock(
        VectorStock::new()
            .with_name("Stock2")
            .with_code("S2")
            .with_low_capacity(10.)
            .with_max_capacity(400.),
        Mailbox::new()
    );

    let mut stock3 = ComponentModel::F64Stock(
        VectorStock::new()
            .with_name("Stock3")
            .with_code("S3")
            .with_low_capacity(10.)
            .with_max_capacity(1000.),
        Mailbox::new()
    );

    connect_components!(&mut stock1, &mut splitter).unwrap();
    connect_components!(&mut splitter, &mut stock2, 0).unwrap();
    connect_components!(&mut splitter, &mut stock3, 1).unwrap();

    let mut stock_logger = ComponentLogger::VectorStockLoggerF64(VectorStockLogger::new("StockLogger".into()));
    let mut process_logger = ComponentLogger::VectorProcessLoggerF64(VectorProcessLogger::new("ProcessLogger".into()));

    connect_logger!(&mut stock_logger, &mut stock1).unwrap();
    connect_logger!(&mut stock_logger, &mut stock2).unwrap();
    connect_logger!(&mut stock_logger, &mut stock3).unwrap();
    connect_logger!(&mut process_logger, &mut splitter).unwrap();
    
    let mut sim_builder = SimInit::new();
    sim_builder = register_component!(sim_builder, stock1);
    sim_builder = register_component!(sim_builder, stock2);
    sim_builder = register_component!(sim_builder, stock3);
    sim_builder = register_component!(sim_builder, splitter);

    let mut simu = sim_builder.init(MonotonicTime::EPOCH).unwrap().0;

    simu.step_until(MonotonicTime::EPOCH + Duration::from_secs(3600)).unwrap();

    let output_dir = "outputs/delay_testing";
    create_dir_all(output_dir).unwrap();
    stock_logger.write_csv(output_dir).unwrap();
    process_logger.write_csv(output_dir).unwrap();
}