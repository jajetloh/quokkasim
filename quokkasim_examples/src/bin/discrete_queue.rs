use std::{error::Error, fs::create_dir_all, time::Duration};
use quokkasim::{define_model_enums, prelude::*};


define_model_enums! {
    pub enum ComponentModel {}
    pub enum ComponentModelAddress {}
    pub enum ComponentLogger {}
    // pub enum ComponentInit {}
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

    let mut df = DistributionFactory {
        base_seed: 1234,
        next_seed: 0,
    };

    let mut source = ComponentModel::DiscreteSourceString(DiscreteSource::new().with_name("Source".into()).with_process_time_distr(Distribution::Constant(3.)), Mailbox::new());
    let mut source_addr = source.get_address();

    let mut queue_1 = ComponentModel::DiscreteStockString(DiscreteStock::new()
        .with_name("Queue1".into())
        .with_low_capacity(0)
        .with_max_capacity(10)
        // .with_initial_contents((0..12).into_iter().map(|i| format!("Person_{:0>2}", i)).collect()),
        .with_initial_contents(Vec::new()),
        Mailbox::new()
    );

    let mut process_1 = ComponentModel::DiscreteProcessString(DiscreteProcess::new()
        .with_name("Process1".into())
        .with_process_time_distr(df.create(DistributionConfig::Triangular { min: 1., max: 10., mode: 6. }).unwrap()),
        Mailbox::new()
    );
    let mut process_1_addr = process_1.get_address();

    let mut queue_2 = ComponentModel::DiscreteStockString(DiscreteStock::new()
        .with_name("Queue2".into())
        .with_low_capacity(0)
        .with_max_capacity(10)
        .with_initial_contents(Vec::new()),
        Mailbox::new()
    );

    let mut sink = ComponentModel::DiscreteSinkString(DiscreteSink::new()
        .with_name("Sink".into())
        .with_process_time_distr(Distribution::Constant(1.)),
        Mailbox::new()
    );
    let mut sink_addr = sink.get_address();

    connect_components!(&mut source, &mut queue_1).unwrap();
    connect_components!(&mut queue_1, &mut process_1).unwrap();
    connect_components!(&mut process_1, &mut queue_2).unwrap();
    connect_components!(&mut queue_2, &mut sink).unwrap();
    // connect_components!(&mut process_2, &mut queue_1).unwrap();

    let mut queue_logger = ComponentLogger::DiscreteStockLoggerString(DiscreteStockLogger::new("QueueLogger".into()));
    let mut process_logger = ComponentLogger::DiscreteProcessLoggerString(DiscreteProcessLogger::new("ProcessLogger".into()));

    connect_logger!(&mut queue_logger, &mut queue_1).unwrap();
    connect_logger!(&mut queue_logger, &mut queue_2).unwrap();
    connect_logger!(&mut process_logger, &mut source).unwrap();
    connect_logger!(&mut process_logger, &mut process_1).unwrap();
    connect_logger!(&mut process_logger, &mut sink).unwrap();

    let mut sim_builder = SimInit::new();
    // let mut init_configs: Vec<ComponentInit> = Vec::new();
    // sim_builder = register_component!(sim_builder, &mut init_configs, queue_1);
    // sim_builder = register_component!(sim_builder, &mut init_configs, process_1);
    // sim_builder = register_component!(sim_builder, &mut init_configs, queue_2);
    // sim_builder = register_component!(sim_builder, &mut init_configs, process_2);
    sim_builder = register_component!(sim_builder, source);
    sim_builder = register_component!(sim_builder, queue_1);
    sim_builder = register_component!(sim_builder, process_1);
    sim_builder = register_component!(sim_builder, queue_2);
    sim_builder = register_component!(sim_builder, sink);

    let mut simu = sim_builder.init(MonotonicTime::EPOCH).unwrap().0;

    source_addr.initialise(&mut simu).unwrap();
    process_1_addr.initialise(&mut simu).unwrap();
    sink_addr.initialise(&mut simu).unwrap();

    simu.step_until(MonotonicTime::EPOCH + Duration::from_secs(200)).unwrap();

    let output_dir = "outputs/discrete_queue";
    create_dir_all(output_dir).unwrap();
    queue_logger.write_csv(output_dir.into()).unwrap();
    process_logger.write_csv(output_dir.into()).unwrap();
}