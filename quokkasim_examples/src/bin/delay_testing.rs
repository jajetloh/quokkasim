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

    // let mut stock1 = ComponentModel::F64Stock(
    //     VectorStock::new()
    //         .with_name("Stock1"), ())

    // let mut queue_logger = ComponentLogger::StringStockLogger(DiscreteStockLogger::new("QueueLogger".into()));
    // let mut process_logger = ComponentLogger::StringProcessLogger(DiscreteProcessLogger::new("ProcessLogger".into()));

    // connect_logger!(&mut queue_logger, &mut queue_1).unwrap();
    // connect_logger!(&mut queue_logger, &mut queue_2).unwrap();
    // connect_logger!(&mut queue_logger, &mut queue_3).unwrap();
    // connect_logger!(&mut process_logger, &mut source).unwrap();
    // connect_logger!(&mut process_logger, &mut process_1).unwrap();
    // connect_logger!(&mut process_logger, &mut process_par).unwrap();
    // connect_logger!(&mut process_logger, &mut sink).unwrap();

    // let mut sim_builder = SimInit::new();
    // sim_builder = register_component!(sim_builder, source);
    // sim_builder = register_component!(sim_builder, queue_1);
    // sim_builder = register_component!(sim_builder, process_1);
    // sim_builder = register_component!(sim_builder, queue_2);
    // sim_builder = register_component!(sim_builder, process_par);
    // sim_builder = register_component!(sim_builder, queue_3);
    // sim_builder = register_component!(sim_builder, sink);

    // let mut simu = sim_builder.init(MonotonicTime::EPOCH).unwrap().0;

    // simu.step_until(MonotonicTime::EPOCH + Duration::from_secs(200)).unwrap();

    // let output_dir = "outputs/discrete_queue";
    // create_dir_all(output_dir).unwrap();
    // queue_logger.write_csv(output_dir).unwrap();
    // process_logger.write_csv(output_dir).unwrap();
}