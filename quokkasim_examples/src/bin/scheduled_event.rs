use std::{error::Error, fs::create_dir_all, time::Duration};

use nexosim::time::MonotonicTime;
use quokkasim::{define_model_enums, prelude::*};

define_model_enums! {
    pub enum ComponentModel {}
    pub enum ComponentModelAddress {}
    pub enum ComponentLogger {}
    pub enum ComponentInit {}
    pub enum ScheduledEventConfig {}
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

impl CustomInit for ComponentInit {
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

    let mut stock_1 = ComponentModel::VectorStockF64(
        VectorStock::new()
            .with_name("Stock 1".into())
            .with_low_capacity(50.)
            .with_max_capacity(101.)
            .with_initial_vector(100.),
        Mailbox::new()
    );
    let mut stock_2 = ComponentModel::VectorStockF64(
        VectorStock::new()
            .with_name("Stock 2".into())
            .with_low_capacity(50.)
            .with_max_capacity(101.)
            .with_initial_vector(100.),
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
    connect_components!(&mut stock_1, &mut process);
    connect_components!(&mut process, &mut stock_2);

    // Connect loggers
    connect_logger!(&mut process_logger, &mut process);
    connect_logger!(&mut stock_logger, &mut stock_1);
    connect_logger!(&mut stock_logger, &mut stock_2);

    // Create simulation
    let mut sim_builder = SimInit::new();
    let mut init_configs: Vec<ComponentInit> = Vec::new();

    sim_builder = register_component!(sim_builder, &mut init_configs, stock_1);
    sim_builder = register_component!(sim_builder, &mut init_configs, stock_2);
    sim_builder = register_component!(sim_builder, &mut init_configs, process);

    let start_time = MonotonicTime::try_from_date_time(2025, 1, 1, 0, 0, 0, 0).unwrap();
    let (mut simu, mut scheduler) = sim_builder.init(start_time.clone()).unwrap();

    let scheduled_events = vec![
        ScheduledEvent::VectorStockF64LowCapacityChange(
            time: start_time.clone() + Duration::from_secs(60),


        )
    ];

    schedule_event!()

}