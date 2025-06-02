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

impl CustomInit for ComponentModelAddress {
    fn initialise(&mut self, simu: &mut Simulation) -> Result<(), ExecutionError> {
        let notif_meta = NotificationMetadata {
            time: simu.time(),
            element_from: "Init".into(),
            message: "Start".into(),
        };
        match self {
            _ => {
                Err(ExecutionError::BadQuery)
            }
        }
    }
}

fn main() {
    // Declarations

    let mut process_logger = ComponentLogger::VectorProcessLoggerVector3(VectorProcessLogger::new("ProcessLogger".into()));
    let mut stock_logger = ComponentLogger::VectorStockLoggerVector3(VectorStockLogger::new("StockLogger".into()));
    let mut df = DistributionFactory {
        base_seed: 1234,
        next_seed: 0,
    };

    let mut stockpile_1 = ComponentModel::VectorStockVector3(
        VectorStock::new()
            .with_name("Stockpile 1".into())
            .with_low_capacity(100.)
            .with_max_capacity(10_000.)
            .with_initial_vector([8000., 2000., 0.].into()),
        Mailbox::new()
    );

    let mut stockpile_2 = ComponentModel::VectorStockVector3(
        VectorStock::new()
            .with_name("Stockpile 2".into())
            .with_low_capacity(100.)
            .with_max_capacity(10_000.)
            .with_initial_vector([0., 6000., 2000.].into()),
        Mailbox::new()
    );

    let mut stockpile_3 = ComponentModel::VectorStockVector3(
        VectorStock::new()
            .with_name("Stockpile 3".into())
            .with_low_capacity(100.)
            .with_max_capacity(10_000.)
            .with_initial_vector([5000., 5000., 0.].into()),
        Mailbox::new()
    );

    let mut reclaimer_1 = ComponentModel::VectorCombiner2Vector3(
        VectorCombiner::new()
            .with_name("Reclaimer 1".into())
            .with_process_quantity_distr(Distribution::Constant(100.))
            .with_process_time_distr(Distribution::Constant(30.)),
        Mailbox::new()
    );

    let mut output_stockpile_1 = ComponentModel::VectorStockVector3(
        VectorStock::new()
            .with_name("Output Stockpile 1".into())
            .with_low_capacity(100.)
            .with_max_capacity(15_000.)
            .with_initial_vector([0., 0., 0.].into()),
        Mailbox::new()
    );

    let mut output_stockpile_2 = ComponentModel::VectorStockVector3(
        VectorStock::new()
            .with_name("Output Stockpile 2".into())
            .with_low_capacity(100.)
            .with_max_capacity(15_000.)
            .with_initial_vector([0., 0., 0.].into()),
        Mailbox::new()
    );

    let mut reclaimer_2 = ComponentModel::VectorCombiner2Vector3(
        VectorCombiner::new()
            .with_name("Reclaimer 2".into())
            .with_process_quantity_distr(Distribution::Constant(100.))
            .with_process_time_distr(Distribution::Constant(30.)),
        Mailbox::new()
    );

    let mut reclaimer_3 = ComponentModel::VectorCombiner1Vector3(
        VectorCombiner::new() 
            .with_name("Reclaimer 3".into())
            .with_process_quantity_distr(Distribution::Constant(100.))
            .with_process_time_distr(Distribution::Constant(60.)),
        Mailbox::new()
    );

    let mut stacker = ComponentModel::VectorSplitter2Vector3(
        VectorSplitter::new()
            .with_name("Stacker".into())
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
    let (mut simu, mut scheduler) = sim_builder.init(start_time).unwrap();

    simu.step_until(start_time + Duration::from_secs(60 * 60 * 1))
        .unwrap();

    let output_dir= "outputs/material_blending";
    create_dir_all(output_dir).unwrap();

    process_logger.write_csv(output_dir.into()).unwrap();
    stock_logger.write_csv(output_dir.into()).unwrap();
}
