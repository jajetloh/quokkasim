#![allow(clippy::manual_async_fn)]

use std::{error::Error, fs::create_dir_all, time::Duration};

use nexosim::time::MonotonicTime;
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
    // Declarations

    let mut process_logger = ComponentLogger::Vector3ProcessLogger(VectorProcessLogger::new("ProcessLogger".into()));
    let mut stock_logger = ComponentLogger::Vector3StockLogger(VectorStockLogger::new("StockLogger".into()));
    let df = DistributionFactory {
        base_seed: 1234,
        next_seed: 0,
    };

    let mut stockpile_1 = ComponentModel::Vector3Stock(
        VectorStock::new()
            .with_name("Stockpile 1")
            .with_code("SP1") 
            .with_low_capacity(100.)
            .with_max_capacity(10_000.)
            .with_initial_resource([8000., 2000., 0.].into()),
        Mailbox::new()
    );

    let mut stockpile_2 = ComponentModel::Vector3Stock(
        VectorStock::new()
            .with_name("Stockpile 2")
            .with_code("SP2") 
            .with_low_capacity(100.)
            .with_max_capacity(10_000.)
            .with_initial_resource([0., 6000., 2000.].into()),
        Mailbox::new()
    );

    let mut stockpile_3 = ComponentModel::Vector3Stock(
        VectorStock::new()
            .with_name("Stockpile 3")
            .with_code("SP3") 
            .with_low_capacity(100.)
            .with_max_capacity(10_000.)
            .with_initial_resource([5000., 5000., 0.].into()),
        Mailbox::new()
    );

    let mut reclaimer_1 = ComponentModel::Vector3Combiner2(
        VectorCombiner::new()
            .with_name("Reclaimer 1")
            .with_code("RC1") 
            .with_process_quantity_distr(Distribution::Constant(100.))
            .with_process_time_distr(Distribution::Constant(30.)),
        Mailbox::new()
    );

    let mut output_stockpile_1 = ComponentModel::Vector3Stock(
        VectorStock::new()
            .with_name("Output Stockpile 1")
            .with_code("OSP1") 
            .with_low_capacity(100.)
            .with_max_capacity(15_000.)
            .with_initial_resource([0., 0., 0.].into()),
        Mailbox::new()
    );

    let mut output_stockpile_2 = ComponentModel::Vector3Stock(
        VectorStock::new()
            .with_name("Output Stockpile 2")
            .with_code("OSP2") 
            .with_low_capacity(100.)
            .with_max_capacity(15_000.)
            .with_initial_resource([0., 0., 0.].into()),
        Mailbox::new()
    );

    let mut reclaimer_2 = ComponentModel::Vector3Combiner2(
        VectorCombiner::new()
            .with_name("Reclaimer 2")
            .with_code("RC2") 
            .with_process_quantity_distr(Distribution::Constant(100.))
            .with_process_time_distr(Distribution::Constant(30.)),
        Mailbox::new()
    );

    let mut reclaimer_3 = ComponentModel::Vector3Combiner1(
        VectorCombiner::new() 
            .with_name("Reclaimer 3")
            .with_code("RC3") 
            .with_process_quantity_distr(Distribution::Constant(100.))
            .with_process_time_distr(Distribution::Constant(60.)),
        Mailbox::new()
    );

    let mut stacker = ComponentModel::Vector3Splitter2(
        VectorSplitter::new()
            .with_name("Stacker")
            .with_code("STK") 
            .with_process_quantity_distr(Distribution::Constant(600.))
            .with_process_time_distr(Distribution::Constant(360.)),
        Mailbox::new()
    );

    connect_components!(&mut stockpile_1, &mut reclaimer_1, 0).unwrap();
    connect_components!(&mut stockpile_2, &mut reclaimer_1, 1).unwrap();
    connect_components!(&mut reclaimer_1, &mut output_stockpile_1).unwrap();

    connect_components!(&mut stockpile_2, &mut reclaimer_2, 0).unwrap();
    connect_components!(&mut stockpile_3, &mut reclaimer_2, 1).unwrap();
    connect_components!(&mut reclaimer_2, &mut output_stockpile_2).unwrap();

    connect_components!(&mut stockpile_1, &mut reclaimer_3, 0).unwrap();
    connect_components!(&mut reclaimer_3, &mut output_stockpile_1).unwrap();

    connect_components!(&mut output_stockpile_1, &mut stacker).unwrap();
    connect_components!(&mut stacker, &mut stockpile_2, 0).unwrap();
    connect_components!(&mut stacker, &mut stockpile_3, 1).unwrap();

    connect_logger!(&mut process_logger, &mut reclaimer_1).unwrap();
    connect_logger!(&mut process_logger, &mut reclaimer_2).unwrap();
    connect_logger!(&mut process_logger, &mut reclaimer_3).unwrap();
    connect_logger!(&mut process_logger, &mut stacker).unwrap();

    connect_logger!(&mut stock_logger, &mut stockpile_1).unwrap();
    connect_logger!(&mut stock_logger, &mut stockpile_2).unwrap();
    connect_logger!(&mut stock_logger, &mut stockpile_3).unwrap();
    connect_logger!(&mut stock_logger, &mut output_stockpile_1).unwrap();
    connect_logger!(&mut stock_logger, &mut output_stockpile_2).unwrap();

    let mut sim_builder = SimInit::new();

    sim_builder = register_component!(sim_builder, reclaimer_1);
    sim_builder = register_component!(sim_builder, reclaimer_2);
    sim_builder = register_component!(sim_builder, reclaimer_3);
    sim_builder = register_component!(sim_builder, stacker);
    sim_builder = register_component!(sim_builder, stockpile_1);
    sim_builder = register_component!(sim_builder, stockpile_2);
    sim_builder = register_component!(sim_builder, stockpile_3);
    sim_builder = register_component!(sim_builder, output_stockpile_1);
    sim_builder = register_component!(sim_builder, output_stockpile_2);

    let start_time = MonotonicTime::try_from_date_time(2025, 1, 1, 0, 0, 0, 0).unwrap();
    let (mut simu, scheduler) = sim_builder.init(start_time).unwrap();

    simu.step_until(start_time + Duration::from_secs(60 * 60))
        .unwrap();

    let output_dir= "outputs/material_blending";
    create_dir_all(output_dir).unwrap();

    process_logger.write_csv(output_dir).unwrap();
    stock_logger.write_csv(output_dir).unwrap();
}
