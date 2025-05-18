use std::{error::Error, fs::create_dir_all, time::Duration};
use quokkasim::{define_model_enums, prelude::*};


define_model_enums! {
    pub enum ComponentModel {}
    pub enum ComponentLogger {}
    pub enum ComponentInit {}
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

    let mut df = DistributionFactory {
        base_seed: 1234,
        next_seed: 0,
    };

    let mut queue_1 = ComponentModel::SequenceStockString(SequenceStock::new()
        .with_name("Queue1".into())
        .with_low_capacity(0)
        .with_max_capacity(10)
        .with_initial_contents((0..12).into_iter().map(|i| format!("Person_{:0>2}", i)).collect()),
        Mailbox::new()
    );

    let mut process_1 = ComponentModel::SequenceProcessString(SequenceProcess::new()
        .with_name("Process1".into())
        .with_process_time_distr(df.create(DistributionConfig::Triangular { min: 1., max: 10., mode: 6. }).unwrap()),
        Mailbox::new()
    );

    let mut queue_2 = ComponentModel::SequenceStockString(SequenceStock::new()
        .with_name("Queue2".into())
        .with_low_capacity(0)
        .with_max_capacity(10)
        .with_initial_contents(Vec::new()),
        Mailbox::new()
    );

    let mut process_2 = ComponentModel::SequenceProcessString(SequenceProcess::new()
        .with_name("Process2".into())
        .with_process_time_distr(df.create(DistributionConfig::Uniform { min: 3., max: 7. }).unwrap()),
        Mailbox::new()
    );

    connect_components!(&mut queue_1, &mut process_1).unwrap();
    connect_components!(&mut process_1, &mut queue_2).unwrap();
    connect_components!(&mut queue_2, &mut process_2).unwrap();
    connect_components!(&mut process_2, &mut queue_1).unwrap();

    let mut queue_logger = ComponentLogger::SequenceStockLoggerString(SequenceStockLogger::new("QueueLogger".into()));
    let mut process_logger = ComponentLogger::SequenceProcessLoggerString(SequenceProcessLogger::new("ProcessLogger".into()));

    connect_logger!(&mut queue_logger, &mut queue_1).unwrap();
    connect_logger!(&mut queue_logger, &mut queue_2).unwrap();
    connect_logger!(&mut process_logger, &mut process_1).unwrap();
    connect_logger!(&mut process_logger, &mut process_2).unwrap();

    let mut sim_builder = SimInit::new();
    let mut init_configs: Vec<ComponentInit> = Vec::new();
    sim_builder = register_component!(sim_builder, &mut init_configs, queue_1);
    sim_builder = register_component!(sim_builder, &mut init_configs, process_1);
    sim_builder = register_component!(sim_builder, &mut init_configs, queue_2);
    sim_builder = register_component!(sim_builder, &mut init_configs, process_2);

    let mut simu = sim_builder.init(MonotonicTime::EPOCH).unwrap().0;

    init_configs.iter_mut().for_each(|x| {
        x.initialise(&mut simu).unwrap();
    });

    simu.step_until(MonotonicTime::EPOCH + Duration::from_secs(200)).unwrap();

    let output_dir = "outputs/discrete_queue";
    create_dir_all(output_dir).unwrap();
    queue_logger.write_csv(output_dir.into()).unwrap();
    process_logger.write_csv(output_dir.into()).unwrap();
}