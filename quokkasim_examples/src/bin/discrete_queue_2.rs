use std::time::Duration;

use nexosim::model::{Context, Model};
use nexosim::ports::{Output, Requestor};
use nexosim::time::MonotonicTime;
use quokkasim::components::queue::{MyQueueProcess, MyQueueSink, MyQueueSource, MyQueueStock, QueueState, MyQueueCombinerProcess};
use quokkasim::common::EventLogger;
use quokkasim::core::{Distribution, DistributionConfig, DistributionFactory, EventLog, NotificationMetadata, Process, SimInit, Sink, Source, Stock};
use nexosim::simulation::Mailbox;


fn main() {
    let logger = EventLogger::new(100_000);

    let mut df = DistributionFactory { base_seed: 1234, next_seed: 0 };

    let mut source1 = MyQueueSource::new()
        .with_name("Source1".into())
        .with_time_to_new_dist(df.create(DistributionConfig::Constant(500.0)).unwrap())
        .with_log_consumer(&logger);
    source1.next_id = 50;
    let source1_mbox: Mailbox<MyQueueSource> = Mailbox::new();
    let source1_addr = source1_mbox.address();

    let mut stock1 = MyQueueStock::new()
        .with_name("Stock1".into())
        .with_log_consumer(&logger);
    let stock1_mbox: Mailbox<MyQueueStock> = Mailbox::new();
    let stock1_addr = stock1_mbox.address();
    stock1.low_capacity = 0;
    stock1.max_capacity = 100;
    stock1.resource.queue = (0..50).into_iter().collect::<Vec<i32>>();

    let mut source2 = MyQueueSource::new()
        .with_name("Source2".into())
        .with_time_to_new_dist(df.create(DistributionConfig::Constant(10.)).unwrap())
        .with_log_consumer(&logger);
    let source2_mbox: Mailbox<MyQueueSource> = Mailbox::new();
    let source2_addr = source2_mbox.address();
    source2.next_id = 1001;

    let mut stock2 = MyQueueStock::new()
        .with_name("Stock2".into())
        .with_log_consumer(&logger);
    let stock2_mbox: Mailbox<MyQueueStock> = Mailbox::new();
    let stock2_addr = stock2_mbox.address();
    stock2.low_capacity = 0;
    stock2.max_capacity = 100;
    
    let mut combiner = MyQueueCombinerProcess::new()
        .with_name("Combiner".into())
        .with_log_consumer(&logger);
    combiner.process_duration_secs_dist = Some(Distribution::Constant(30.));
    combiner.process_quantity_dist = Some(Distribution::Constant(2.));
    let combiner_mbox: Mailbox<MyQueueCombinerProcess> = Mailbox::new();
    let combiner_addr = combiner_mbox.address();

    let mut process = MyQueueProcess::new()
        .with_name("Process".into())
        .with_log_consumer(&logger);
    process.process_quantity_dist = Some(Distribution::Constant(1.));
    process.process_duration_secs_dist = Some(Distribution::Constant(15.));
    let process_mbox: Mailbox<MyQueueProcess> = Mailbox::new();
    let process_addr = process_mbox.address();

    let mut stock3 = MyQueueStock::new()
        .with_name("Stock3".into())
        .with_log_consumer(&logger);
    let stock3_mbox: Mailbox<MyQueueStock> = Mailbox::new();
    let stock3_addr = stock3_mbox.address();
    stock3.low_capacity = 0;
    stock3.max_capacity = 10;

    let mut sink= MyQueueSink::new()
        .with_name("Sink".into())
        .with_time_to_destroy_dist(Distribution::Constant( 1. ))
        .with_log_consumer(&logger);
    let sink_mbox: Mailbox<MyQueueSink> = Mailbox::new();
    let sink_addr = sink_mbox.address();

    source1.push_downstream.connect(MyQueueStock::add, &stock1_addr);
    source1.req_downstream.connect(MyQueueStock::get_state, &stock1_addr);
    stock1.state_emitter.connect(MyQueueSource::check_update_state, &source1_addr);

    source2.push_downstream.connect(MyQueueStock::add, &stock2_addr);
    source2.req_downstream.connect(MyQueueStock::get_state, &stock2_addr);
    stock2.state_emitter.connect(MyQueueSource::check_update_state, &source2_addr);

    combiner.withdraw_upstreams.0.connect(MyQueueStock::remove, &stock1_addr);
    combiner.withdraw_upstreams.1.connect(MyQueueStock::remove, &stock2_addr);
    combiner.req_upstreams.0.connect(MyQueueStock::get_state, &stock1_addr);
    combiner.req_upstreams.1.connect(MyQueueStock::get_state, &stock2_addr);
    stock1.state_emitter.connect(MyQueueCombinerProcess::check_update_state, &combiner_addr);
    stock2.state_emitter.connect(MyQueueCombinerProcess::check_update_state, &combiner_addr);
    combiner.push_downstream.connect(MyQueueStock::add, &stock3_addr);
    combiner.req_downstream.connect(MyQueueStock::get_state, &stock3_addr);
    stock3.state_emitter.connect(MyQueueCombinerProcess::check_update_state, &combiner_addr);

    process.withdraw_upstream.connect(MyQueueStock::remove, &stock1_addr);
    process.req_upstream.connect(MyQueueStock::get_state, &stock1_addr);
    stock1.state_emitter.connect(MyQueueProcess::check_update_state, &process_addr);
    process.push_downstream.connect(MyQueueStock::add, &stock2_addr);
    process.req_downstream.connect(MyQueueStock::get_state, &stock2_addr);
    stock3.state_emitter.connect(MyQueueProcess::check_update_state, &process_addr);

    sink.withdraw_upstream.connect(MyQueueStock::remove, &stock3_addr);
    sink.req_upstream.connect(MyQueueStock::get_state, &stock3_addr);
    stock3.state_emitter.connect(MyQueueSink::check_update_state, &sink_addr);

    let sim_builder = SimInit::new()
        .add_model(source1, source1_mbox, "Source1")
        .add_model(stock1, stock1_mbox, "Stock1")
        .add_model(source2, source2_mbox, "Source2")
        .add_model(stock2, stock2_mbox, "Stock2")
        .add_model(combiner, combiner_mbox, "Combiner")
        .add_model(process, process_mbox, "Process")
        .add_model(stock3, stock3_mbox, "Stock3")
        .add_model(sink, sink_mbox, "Sink");

    let mut simu = sim_builder.init(MonotonicTime::EPOCH).unwrap().0;
    simu.process_event(
        MyQueueSource::check_update_state,
        NotificationMetadata {
            time: MonotonicTime::EPOCH,
            element_from: "Init".into(),
            message: "check_update_state".into(),
        },
        &source1_addr,
    ).unwrap();
    simu.process_event(
        MyQueueSource::check_update_state,
        NotificationMetadata {
            time: MonotonicTime::EPOCH,
            element_from: "Init".into(),
            message: "check_update_state".into(),
        },
        &source2_addr,
    ).unwrap();
    simu.process_event(MyQueueCombinerProcess::check_update_state, NotificationMetadata {
        time: MonotonicTime::EPOCH,
        element_from: "Init".into(),
        message: "check_update_state".into(),
    }, &combiner_addr).unwrap();
    simu.process_event(MyQueueProcess::check_update_state, NotificationMetadata {
        time: MonotonicTime::EPOCH,
        element_from: "Init".into(),
        message: "check_update_state".into(),
    }, &process_addr).unwrap();
    simu.step_until(MonotonicTime::EPOCH + Duration::from_secs(200)).unwrap();

    logger.write_csv("outputs/discrete_queue_2.csv").unwrap();
}