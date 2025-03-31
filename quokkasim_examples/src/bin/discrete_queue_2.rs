use std::time::Duration;

use nexosim::time::MonotonicTime;
use quokkasim::components::{MyQueueProcess, MyQueueSink, MyQueueSource, MyQueueStock};
use quokkasim::common::EventLogger;
use quokkasim::core::{Distribution, DistributionConfig, DistributionFactory, NotificationMetadata, Process, SimInit, Sink, Source, Stock};
use nexosim::simulation::Mailbox;

fn main() {
    let logger = EventLogger::new(100_000);

    let mut df = DistributionFactory { base_seed: 1234, next_seed: 0 };

    let mut source = MyQueueSource::new()
        .with_name("Source".into())
        .with_time_to_new_dist(df.create(DistributionConfig::Exponential { mean: 1.5 }).unwrap())
        .with_log_consumer(&logger);
    let source_mbox: Mailbox<MyQueueSource> = Mailbox::new();
    let source_addr = source_mbox.address();

    let mut stock1 = MyQueueStock::new()
        .with_name("Stock".into())
        .with_log_consumer(&logger);
    let stock1_mbox: Mailbox<MyQueueStock> = Mailbox::new();
    let stock1_addr = stock1_mbox.address();
    stock1.low_capacity = 0;
    stock1.max_capacity = 10;

    let mut process = MyQueueProcess::new()
        .with_name("Process".into())
        .with_log_consumer(&logger);
    let process_mbox: Mailbox<MyQueueProcess> = Mailbox::new();
    let process_addr = process_mbox.address();

    let mut stock2 = MyQueueStock::new()
        .with_name("Stock2".into())
        .with_log_consumer(&logger);
    let stock2_mbox: Mailbox<MyQueueStock> = Mailbox::new();
    let stock2_addr = stock2_mbox.address();
    stock2.low_capacity = 0;
    stock2.max_capacity = 10;

    let mut sink= MyQueueSink::new()
        .with_name("Sink".into())
        .with_time_to_destroy_dist(Distribution::Constant( 0.1 ))
        .with_log_consumer(&logger);
    let sink_mbox: Mailbox<MyQueueSink> = Mailbox::new();
    let sink_addr = sink_mbox.address();

    source.push_downstream.connect(MyQueueStock::add, &stock1_addr);
    source.req_downstream.connect(MyQueueStock::get_state, &stock1_addr);
    stock1.state_emitter.connect(MyQueueSource::check_update_state, &source_addr);

    process.withdraw_upstream.connect(MyQueueStock::remove, &stock1_addr);
    process.req_upstream.connect(MyQueueStock::get_state, &stock1_addr);
    stock1.state_emitter.connect(MyQueueProcess::check_update_state, &process_addr);

    process.push_downstream.connect(MyQueueStock::add, &stock2_addr);
    process.req_downstream.connect(MyQueueStock::get_state, &stock2_addr);
    stock2.state_emitter.connect(MyQueueProcess::check_update_state, &process_addr);

    sink.withdraw_upstream.connect(MyQueueStock::remove, &stock2_addr);
    sink.req_upstream.connect(MyQueueStock::get_state, &stock2_addr);
    stock2.state_emitter.connect(MyQueueSink::check_update_state, &sink_addr);

    let sim_builder = SimInit::new()
        .add_model(source, source_mbox, "Source")
        .add_model(stock1, stock1_mbox, "Stock")
        .add_model(process, process_mbox, "Process")
        .add_model(stock2, stock2_mbox, "Stock2")
        .add_model(sink, sink_mbox, "Sink");

    let mut simu = sim_builder.init(MonotonicTime::EPOCH).unwrap().0;
    simu.process_event(
        MyQueueSource::check_update_state,
        NotificationMetadata {
            time: MonotonicTime::EPOCH,
            element_from: "Source".into(),
            message: "check_update_state".into(),
        },
        &source_addr,
    ).unwrap();
    simu.step_until(MonotonicTime::EPOCH + Duration::from_secs(100)).unwrap();

    logger.write_csv("logs.csv").unwrap();
}