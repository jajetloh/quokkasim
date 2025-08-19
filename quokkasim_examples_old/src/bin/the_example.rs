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

    let mut df = DistributionFactory::new(12345);

    let mut rock_source_1 = ComponentModel::Vector3Source(
        VectorSource::new()
            .with_name("Rock Source 1")
            .with_code("RS1")
            .with_process_time_distr(Distribution::Constant(600.))
            .with_process_quantity_distr(Distribution::Constant(5000.))
            .with_source_vector([300., 100., 0.].into()),
        Mailbox::new()
    );

    let mut rock_source_2 = ComponentModel::Vector3Source(
        VectorSource::new()
            .with_name("Rock Source 2")
            .with_code("RS2")
            .with_process_time_distr(Distribution::Constant(300.))
            .with_process_quantity_distr(Distribution::Constant(2000.))
            .with_source_vector([200., 200., 0.].into()),
        Mailbox::new()
    );

    let mut rock_stock_1 = ComponentModel::Vector3Stock(
        VectorStock::new()
            .with_name("Rock Stock 1")
            .with_code("RS1")
            .with_low_capacity(100.)
            .with_max_capacity(2000.)
            .with_initial_resource([1600., 400., 0.].into()),
        Mailbox::new()
    );

    let mut trucking = ComponentModel::Vector3Process(
        VectorProcess::new()
            .with_name("Trucking")
            .with_code("T1")
            .with_process_time_distr(Distribution::Constant(18.))
            .with_process_quantity_distr(Distribution::Constant(60.)),
        Mailbox::new()
    );

    let mut rock_stock_2 = ComponentModel::Vector3Stock(
        VectorStock::new()
            .with_name("Rock Stock 2")
            .with_code("RS2")
            .with_low_capacity(100.)
            .with_max_capacity(2000.)
            .with_initial_resource([1600., 400., 0.].into()),
        Mailbox::new()
    );

    let mut conveyors = ComponentModel::Vector3Splitter2(
        VectorSplitter::new()
            .with_name("Conveyor")
            .with_code("C1")
            .with_process_time_distr(Distribution::Constant(18.))
            .with_process_quantity_distr(Distribution::Constant(60.))
            .with_split_ratios([2./3., 1./3.])
            .with_delay_mode(
                DelayModeChange::Add(DelayMode {
                    name: "Short Delay".to_string(),
                    until_delay_distr: df.create(DistributionConfig::Uniform { min: 60., max: 180. }).unwrap(),
                    until_fix_distr: df.create(DistributionConfig::Uniform { min: 10., max: 60. }).unwrap(),
                })
            ),
        Mailbox::new()
    );

    let mut process_feed_1 = ComponentModel::Vector3Stock(
        VectorStock::new()
            .with_name("Process Feed 1")
            .with_code("PF1")
            .with_low_capacity(100.)
            .with_max_capacity(500.),
        Mailbox::new()
    );

    let mut processing_1 = ComponentModel::Vector3Sink(
        VectorSink::new()
            .with_name("Processing 1")
            .with_code("P1")
            .with_process_time_distr(Distribution::Constant(18.))
            .with_process_quantity_distr(Distribution::Constant(60.)),
        Mailbox::new()
    );

    let mut process_feed_2 = ComponentModel::Vector3Stock(
        VectorStock::new()
            .with_name("Process Feed 2")
            .with_code("PF2")
            .with_low_capacity(100.)
            .with_max_capacity(500.),
        Mailbox::new()
    );

    let mut processing_2 = ComponentModel::Vector3Sink(
        VectorSink::new()
            .with_name("Processing 2")
            .with_code("P2")
            .with_process_time_distr(df.create(DistributionConfig::Uniform { min: 10., max: 26. }).unwrap())
            .with_process_quantity_distr(df.create(DistributionConfig::TruncNormal { mean: 30., std: 10., min: Some(1.), max: None }).unwrap()),
        Mailbox::new()
    );

    connect_components!(&mut rock_source_1, &mut rock_stock_1).unwrap();
    connect_components!(&mut rock_source_2, &mut rock_stock_1).unwrap();
    connect_components!(&mut rock_stock_1, &mut trucking).unwrap();
    connect_components!(&mut trucking, &mut rock_stock_2).unwrap();
    connect_components!(&mut rock_stock_2, &mut conveyors).unwrap();
    connect_components!(&mut conveyors, &mut process_feed_1, 0).unwrap();
    connect_components!(&mut conveyors, &mut process_feed_2, 1).unwrap();
    connect_components!(&mut process_feed_1, &mut processing_1).unwrap();
    connect_components!(&mut process_feed_2, &mut processing_2).unwrap();

    let mut stock_logger = ComponentLogger::Vector3StockLogger(VectorStockLogger::new("VectorStock"));
    let mut process_logger = ComponentLogger::Vector3ProcessLogger(VectorProcessLogger::new("VectorProcess"));

    connect_logger!(&mut process_logger, &mut rock_source_1).unwrap();
    connect_logger!(&mut process_logger, &mut rock_source_2).unwrap();
    connect_logger!(&mut stock_logger, &mut rock_stock_1).unwrap();
    connect_logger!(&mut process_logger, &mut trucking).unwrap();
    connect_logger!(&mut stock_logger, &mut rock_stock_2).unwrap();
    connect_logger!(&mut process_logger, &mut conveyors).unwrap();
    connect_logger!(&mut stock_logger, &mut process_feed_1).unwrap();
    connect_logger!(&mut process_logger, &mut processing_1).unwrap();
    connect_logger!(&mut stock_logger, &mut process_feed_2).unwrap();
    connect_logger!(&mut process_logger, &mut processing_2).unwrap();

    let mut sim_builder = SimInit::new();
    sim_builder = register_component!(sim_builder, rock_source_1);
    sim_builder = register_component!(sim_builder, rock_source_2);
    sim_builder = register_component!(sim_builder, rock_stock_1);
    sim_builder = register_component!(sim_builder, trucking);
    sim_builder = register_component!(sim_builder, rock_stock_2);
    sim_builder = register_component!(sim_builder, conveyors);
    sim_builder = register_component!(sim_builder, process_feed_1);
    sim_builder = register_component!(sim_builder, processing_1);
    sim_builder = register_component!(sim_builder, process_feed_2);
    sim_builder = register_component!(sim_builder, processing_2);


    let start_time = MonotonicTime::try_from_date_time(2025, 6, 22, 0, 0, 0, 0).unwrap();
    let (mut simu, mut scheduler) = sim_builder.init(start_time.clone()).unwrap();

    simu.step_until(start_time + Duration::from_secs(700)).unwrap();
    // simu.step_until(start_time + Duration::from_nanos((8*60+19)*1_000_000_000 + 820759260)).unwrap();

    let output_dir = "outputs/rocks";
    create_dir_all(output_dir).unwrap();
    stock_logger.write_csv(output_dir).unwrap();
    process_logger.write_csv(output_dir).unwrap();
}