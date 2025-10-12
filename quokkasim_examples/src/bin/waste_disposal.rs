use quokkasim::prelude::*;
use std::{time::{Duration, SystemTime}};

fn create_bench() {
    let mut df = DistributionFactory::new(55555);

    // Component declarations

    let mut dump_source: DefaultContSource<f64, ContProcessLog<DefaultContProcessLogType<f64>, f64>> =
        DefaultContSource::new()
            .with_name("DumpSource")
            .with_code("DS")
            .with_source_resource(1.0)
            .with_source_quantity_distr(
                df.create(DistributionConfig::Constant(50.0))
                    .unwrap(),
            );
    let ds_mbox = Mailbox::new();
    let ds_addr = ds_mbox.address();

    let mut dump_point: DefaultContStock<f64, ContStockState, ContStockLog<f64>> = DefaultContStock::new()
        .with_name("DumpPoint")
        .with_code("DP")
        .with_low_capacity(1.)
        .with_max_capacity(10_000.)
        .with_initial_resource(0.);
    let dp_mbox = Mailbox::new();
    let dp_addr = dp_mbox.address();

    let mut material_sink: DefaultContSink<f64, ContProcessLog<DefaultContProcessLogType<f64>, f64>> =
        DefaultContSink::new()
            .with_name("MaterialSink")
            .with_code("MS")
            .with_sink_quantity_distr(
                df.create(DistributionConfig::Constant(50.0))
                    .unwrap(),
            )
            .with_sink_time_distr(df.create(DistributionConfig::Constant(1.0)).unwrap());
    let ms_mbox = Mailbox::new();
    let ms_addr = ms_mbox.address();

    // Connections

    let mut c = Connection {};
    c.connect((&mut dump_source, &ds_addr), (&mut dump_point, &dp_addr)).unwrap();
    c.connect((&mut dump_point, &dp_addr), (&mut material_sink, &ms_addr)).unwrap();

    // Loggers

    let process_logger = EventQueue::<ContProcessLog<DefaultContProcessLogType<f64>, f64>>::new();
    let stock_logger = EventQueue::<ContStockLog<f64>>::new();

    dump_source.log_emitter.connect_sink(&process_logger);
    dump_point.log_emitter.connect_sink(&stock_logger);
    material_sink.log_emitter.connect_sink(&process_logger);

    // Registry

    // Simulation initialisation

    let sim_init = SimInit::new()
        .add_model(dump_source, ds_mbox, "DumpSource")
        .add_model(dump_point, dp_mbox, "DumpPoint")
        .add_model(material_sink, ms_mbox, "MaterialSink");

    let start_time = MonotonicTime::try_from_date_time(2025, 7, 1, 0, 0, 0, 0).unwrap();
    let duration = Duration::from_secs(24 * 3600);
    let (mut sim, sched) = sim_init.init(start_time).unwrap();

    let time_at_start = SystemTime::now();
    sim.step_until(start_time + duration).unwrap();
    let time_at_end = SystemTime::now();
    println!("Execution time: {:?}", time_at_end.duration_since(time_at_start));

    for log in process_logger.into_reader() {
        println!("{:?}", log);
    }

}

fn main() {
    create_bench();
}