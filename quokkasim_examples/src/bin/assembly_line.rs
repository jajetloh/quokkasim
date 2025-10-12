use quokkasim::prelude::*;
use std::{time::{Duration, SystemTime}};

fn create_bench() {
    let mut df = DistributionFactory::new(55555);

    // Component declarations


    let mut source: DefaultDiscSource<String, DiscProcessLog<DefaultDiscProcessLogType<String>, String>> = DefaultDiscSource::new()
        .with_name("Source")
        .with_code("SRC")
        .with_source_time_distr(df.create(DistributionConfig::Constant(0.5)).unwrap());
    let src_mbox = Mailbox::new();
    let src_addr = src_mbox.address();
    source.source_item_generator = Some(Box::new(SimpleStringGenerator::new("ITEM_{}".into())));

    let mut queue_1: DefaultDiscStock<String, DiscStockState, DiscStockLog<String>> = DefaultDiscStock::new()
        .with_name("Queue 1")
        .with_code("Q1")
        .with_max_capacity(10);
    queue_1.resources().add_multi(["A1", "A2", "A3"].iter().map(|s| s.to_string()).collect());

    let q1_mbox = Mailbox::new();
    let q1_addr = q1_mbox.address();

    let mut process: DefaultDiscProcess<String, DiscProcessLog<DefaultDiscProcessLogType<String>, String>> = DefaultDiscProcess::new()
        .with_name("Process")
        .with_code("P")
        .with_process_time_distr(df.create(DistributionConfig::Constant(0.1)).unwrap());
    let p_mbox = Mailbox::new();
    let p_addr = p_mbox.address();

    let mut queue_2: DefaultDiscStock<String, DiscStockState, DiscStockLog<String>> = DefaultDiscStock::new()
        .with_name("Queue 2")
        .with_code("Q2")
        .with_max_capacity(10);
    let q2_mbox = Mailbox::new();
    let q2_addr = q2_mbox.address();

    let mut sink : DefaultDiscSink<String, DiscProcessLog<DefaultDiscProcessLogType<String>, String>> = DefaultDiscSink::new()
        .with_name("Sink")
        .with_code("SNK")
        .with_sink_time_distr(df.create(DistributionConfig::Constant(0.5)).unwrap());
    let snk_mbox = Mailbox::new();
    let snk_addr = snk_mbox.address();

    // Connections

    let mut c = Connection {};
    c.connect((&mut source, &src_addr), (&mut queue_1, &q1_addr)).unwrap();
    c.connect((&mut queue_1, &q1_addr), (&mut process, &p_addr)).unwrap();
    c.connect((&mut process, &p_addr), (&mut queue_2, &q2_addr)).unwrap();
    c.connect((&mut queue_2, &q2_addr), (&mut sink, &snk_addr)).unwrap();

    // Loggers

    let process_logger = EventQueue::<DiscProcessLog<DefaultDiscProcessLogType<String>, String>>::new();
    source.log_emitter.connect_sink(&process_logger);
    process.log_emitter.connect_sink(&process_logger);
    sink.log_emitter.connect_sink(&process_logger);

    let stock_logger = EventQueue::<DiscStockLog<String>>::new();
    queue_1.log_emitter.connect_sink(&stock_logger);
    queue_2.log_emitter.connect_sink(&stock_logger);

    // Registry

    // Simulation initialisation

    let sim_init = SimInit::new()
        .add_model(source, src_mbox, "Source")
        .add_model(queue_1, q1_mbox, "Queue 1")
        .add_model(process, p_mbox, "Process")
        .add_model(queue_2, q2_mbox, "Queue 2")
        .add_model(sink, snk_mbox, "Sink");

    let start_time = MonotonicTime::try_from_date_time(2025, 7, 1, 0, 0, 0, 0).unwrap();
    let duration = Duration::from_secs(3);
    let (mut sim, sched) = sim_init.init(start_time).unwrap();

    let time_at_start = SystemTime::now();
    sim.step_until(start_time + duration).unwrap();
    let time_at_end = SystemTime::now();
    println!("Execution time: {:?}", time_at_end.duration_since(time_at_start));

    for log in process_logger.into_reader() {
        println!("{:?}", log);
    }
    for log in stock_logger.into_reader() {
        println!("{:?}", log);
    }

}

fn main() {
    create_bench();
}