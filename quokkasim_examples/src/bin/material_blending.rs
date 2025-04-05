use std::time::Duration;

use nexosim::time::MonotonicTime;
use quokkasim::{
    common::EventLogger,
    components::array::{ArrayCombinerProcess, ArrayProcess, ArrayStock},
    core::{
        Distribution, DistributionConfig, DistributionFactory, Mailbox, NotificationMetadata,
        Process, SimInit, Stock,
    },
};

fn main() {
    // Declarations

    let logger = EventLogger::new(100_000);
    let mut df = DistributionFactory {
        base_seed: 1234,
        next_seed: 0,
    };

    let mut stockpile_1 = ArrayStock::new()
        .with_name("Stockpile 1".into())
        .with_log_consumer(&logger);
    stockpile_1.low_capacity = 100.;
    stockpile_1.max_capacity = 10000.;
    stockpile_1.resource.vec = [8000., 2000., 0., 0., 0.];

    let stockpile_1_mbox: Mailbox<ArrayStock> = Mailbox::new();
    let stockpile_1_addr = stockpile_1_mbox.address();

    let mut stockpile_2 = ArrayStock::new()
        .with_name("Stockpile 2".into())
        .with_log_consumer(&logger);
    stockpile_2.low_capacity = 100.;
    stockpile_2.max_capacity = 10000.;
    stockpile_2.resource.vec = [0., 6000., 1000., 1000., 0.];

    let stockpile_2_mbox: Mailbox<ArrayStock> = Mailbox::new();
    let stockpile_2_addr = stockpile_2_mbox.address();

    let mut stockpile_3 = ArrayStock::new()
        .with_name("Stockpile 3".into())
        .with_log_consumer(&logger);
    stockpile_3.low_capacity = 100.;
    stockpile_3.max_capacity = 20000.;
    stockpile_3.resource.vec = [5000., 5000., 5000., 5000., 0.];

    let stockpile_3_mbox: Mailbox<ArrayStock> = Mailbox::new();
    let stockpile_3_addr = stockpile_3_mbox.address();

    let mut reclaimer_1 = ArrayCombinerProcess::new()
        .with_name("Reclaimer 1".into())
        .with_log_consumer(&logger);
    reclaimer_1.process_quantity_dist = Some(
        df.create(DistributionConfig::TruncNormal {
            mean: 100.,
            std: 30.,
            min: Some(0.),
            max: None,
        })
        .unwrap(),
    );
    reclaimer_1.process_duration_secs_dist = Some(Distribution::Constant(25.));

    let reclaimer_1_mbox: Mailbox<ArrayCombinerProcess> = Mailbox::new();
    let reclaimer_1_addr = reclaimer_1_mbox.address();

    let mut output_stockpile_1 = ArrayStock::new()
        .with_name("Output Stockpile 1".into())
        .with_log_consumer(&logger);
    output_stockpile_1.low_capacity = 100.;
    output_stockpile_1.max_capacity = 15000.;

    let output_stockpile_1_mbox: Mailbox<ArrayStock> = Mailbox::new();
    let output_stockpile_1_addr = output_stockpile_1_mbox.address();

    let mut reclaimer_2 = ArrayCombinerProcess::new()
        .with_name("Reclaimer 2".into())
        .with_log_consumer(&logger);
    reclaimer_2.process_quantity_dist = Some(
        df.create(DistributionConfig::TruncNormal {
            mean: 80.,
            std: 5.,
            min: Some(0.),
            max: None,
        })
        .unwrap(),
    );
    reclaimer_2.process_duration_secs_dist = Some(Distribution::Constant(35.));

    let reclaimer_2_mbox: Mailbox<ArrayCombinerProcess> = Mailbox::new();
    let reclaimer_2_addr = reclaimer_2_mbox.address();

    let mut output_stockpile_2 = ArrayStock::new()
        .with_name("Output Stockpile 2".into())
        .with_log_consumer(&logger);
    output_stockpile_2.low_capacity = 100.;
    output_stockpile_2.max_capacity = 15000.;

    let output_stockpile_2_mbox: Mailbox<ArrayStock> = Mailbox::new();
    let output_stockpile_2_addr = output_stockpile_2_mbox.address();

    let mut reclaimer_3 = ArrayProcess::new()
        .with_name("Reclaimer 3".into())
        .with_log_consumer(&logger);
    reclaimer_3.process_quantity_dist = Some(Distribution::Constant(100.));
    reclaimer_3.process_duration_secs_dist = Some(Distribution::Constant(60.));
    let reclaimer_3_mbox: Mailbox<ArrayProcess> = Mailbox::new();
    let reclaimer_3_addr = reclaimer_3_mbox.address();

    // Connections

    // - Reclaimer 1:

    reclaimer_1
        .req_upstreams
        .0
        .connect(ArrayStock::get_state, &stockpile_1_addr);
    reclaimer_1
        .withdraw_upstreams
        .0
        .connect(ArrayStock::remove, &stockpile_1_addr);
    stockpile_1
        .state_emitter
        .connect(ArrayCombinerProcess::check_update_state, &reclaimer_1_addr);

    reclaimer_1
        .req_upstreams
        .1
        .connect(ArrayStock::get_state, &stockpile_2_addr);
    reclaimer_1
        .withdraw_upstreams
        .1
        .connect(ArrayStock::remove, &stockpile_2_addr);
    stockpile_2
        .state_emitter
        .connect(ArrayCombinerProcess::check_update_state, &reclaimer_1_addr);

    reclaimer_1
        .push_downstream
        .connect(ArrayStock::add, &output_stockpile_1_addr);
    reclaimer_1
        .req_downstream
        .connect(ArrayStock::get_state, &output_stockpile_1_addr);
    output_stockpile_1
        .state_emitter
        .connect(ArrayCombinerProcess::check_update_state, &reclaimer_1_addr);

    // - Reclaimer 2:

    reclaimer_2
        .req_upstreams
        .0
        .connect(ArrayStock::get_state, &stockpile_2_addr);
    reclaimer_2
        .withdraw_upstreams
        .0
        .connect(ArrayStock::remove, &stockpile_2_addr);
    stockpile_2
        .state_emitter
        .connect(ArrayCombinerProcess::check_update_state, &reclaimer_2_addr);

    reclaimer_2
        .req_upstreams
        .1
        .connect(ArrayStock::get_state, &stockpile_3_addr);
    reclaimer_2
        .withdraw_upstreams
        .1
        .connect(ArrayStock::remove, &stockpile_3_addr);
    stockpile_3
        .state_emitter
        .connect(ArrayCombinerProcess::check_update_state, &reclaimer_2_addr);

    reclaimer_2
        .push_downstream
        .connect(ArrayStock::add, &output_stockpile_2_addr);
    reclaimer_2
        .req_downstream
        .connect(ArrayStock::get_state, &output_stockpile_2_addr);
    output_stockpile_2
        .state_emitter
        .connect(ArrayCombinerProcess::check_update_state, &reclaimer_2_addr);

    // - Reclaimer 3

    reclaimer_3
        .req_upstream
        .connect(ArrayStock::get_state, &stockpile_1_addr);
    reclaimer_3
        .withdraw_upstream
        .connect(ArrayStock::remove, &stockpile_1_addr);
    stockpile_1
        .state_emitter
        .connect(ArrayProcess::check_update_state, &reclaimer_3_addr);

    reclaimer_3
        .req_downstream
        .connect(ArrayStock::get_state, &output_stockpile_1_addr);
    reclaimer_3
        .push_downstream
        .connect(ArrayStock::add, &output_stockpile_1_addr);
    output_stockpile_1
        .state_emitter
        .connect(ArrayProcess::check_update_state, &reclaimer_3_addr);

    // Model compilation and running

    let sim_builder = SimInit::new()
        .add_model(stockpile_1, stockpile_1_mbox, "Stockpile 1")
        .add_model(stockpile_2, stockpile_2_mbox, "Stockpile 2")
        .add_model(stockpile_3, stockpile_3_mbox, "Stockpile 3")
        .add_model(reclaimer_1, reclaimer_1_mbox, "Reclaimer 1")
        .add_model(
            output_stockpile_1,
            output_stockpile_1_mbox,
            "Output Stockpile 1",
        )
        .add_model(reclaimer_2, reclaimer_2_mbox, "Reclaimer 2")
        .add_model(
            output_stockpile_2,
            output_stockpile_2_mbox,
            "Output Stockpile 2",
        )
        .add_model(reclaimer_3, reclaimer_3_mbox, "Reclaimer 3");

    let start_time = MonotonicTime::try_from_date_time(2025, 1, 1, 0, 0, 0, 0).unwrap();
    let mut simu = sim_builder.init(start_time).unwrap().0;
    simu.process_event(
        ArrayCombinerProcess::check_update_state,
        NotificationMetadata {
            time: start_time,
            element_from: "Reclaimer 1".into(),
            message: "Start".into(),
        },
        &reclaimer_1_addr,
    )
    .unwrap();
    simu.process_event(
        ArrayCombinerProcess::check_update_state,
        NotificationMetadata {
            time: start_time,
            element_from: "Reclaimer 2".into(),
            message: "Start".into(),
        },
        &reclaimer_2_addr,
    )
    .unwrap();
    simu.process_event(
        ArrayProcess::check_update_state,
        NotificationMetadata {
            time: start_time,
            element_from: "Reclaimer 3".into(),
            message: "Start".into(),
        },
        &reclaimer_3_addr,
    )
    .unwrap();

    simu.step_until(start_time + Duration::from_secs(60 * 60 * 24 * 2))
        .unwrap();
    logger.write_csv("material_blending.csv").unwrap();
}
