use std::time::Duration;

use nexosim::{ports::EventBuffer, time::MonotonicTime};
use quokkasim::{
    common::Logger,
    components::vector::{VectorCombinerProcess, VectorProcess, VectorProcessLogger, VectorSplitterProcess, VectorStock, VectorStockLogger},
    core::{
        Distribution, DistributionConfig, DistributionFactory, Mailbox, NotificationMetadata, Process, SimInit, Stock
    },
};

fn main() {
    // Declarations

    let logger: VectorProcessLogger = VectorProcessLogger {
        buffer: EventBuffer::with_capacity(100_000),
        name: "ProcessLogger".into(),
    };
    let stock_logger: VectorStockLogger = VectorStockLogger { 
        buffer: EventBuffer::with_capacity(100_000),
        name: "StockLogger".into(),
    };
    let mut df = DistributionFactory {
        base_seed: 1234,
        next_seed: 0,
    };

    let mut stockpile_1 = VectorStock::new()
        .with_name("Stockpile 1".into())
        .with_log_consumer(&stock_logger);
    stockpile_1.low_capacity = 100.;
    stockpile_1.max_capacity = 10000.;
    stockpile_1.resource.vec = [8000., 2000., 0., 0., 0.];

    let stockpile_1_mbox: Mailbox<VectorStock> = Mailbox::new();
    let stockpile_1_addr = stockpile_1_mbox.address();

    let mut stockpile_2 = VectorStock::new()
        .with_name("Stockpile 2".into())
        .with_log_consumer(&stock_logger);
    stockpile_2.low_capacity = 100.;
    stockpile_2.max_capacity = 10000.;
    stockpile_2.resource.vec = [0., 6000., 1000., 1000., 0.];

    let stockpile_2_mbox: Mailbox<VectorStock> = Mailbox::new();
    let stockpile_2_addr = stockpile_2_mbox.address();

    let mut stockpile_3 = VectorStock::new()
        .with_name("Stockpile 3".into())
        .with_log_consumer(&stock_logger);
    stockpile_3.low_capacity = 100.;
    stockpile_3.max_capacity = 20000.;
    stockpile_3.resource.vec = [5000., 5000., 5000., 5000., 0.];

    let stockpile_3_mbox: Mailbox<VectorStock> = Mailbox::new();
    let stockpile_3_addr = stockpile_3_mbox.address();

    let mut reclaimer_1 = VectorCombinerProcess::new()
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

    let reclaimer_1_mbox: Mailbox<VectorCombinerProcess> = Mailbox::new();
    let reclaimer_1_addr = reclaimer_1_mbox.address();

    let mut output_stockpile_1 = VectorStock::new()
        .with_name("Output Stockpile 1".into())
        .with_log_consumer(&stock_logger);
    output_stockpile_1.low_capacity = 100.;
    output_stockpile_1.max_capacity = 15000.;

    let output_stockpile_1_mbox: Mailbox<VectorStock> = Mailbox::new();
    let output_stockpile_1_addr = output_stockpile_1_mbox.address();

    let mut reclaimer_2 = VectorCombinerProcess::new()
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
    reclaimer_2.process_duration_secs_dist = Some(df.create(DistributionConfig::Triangular { min: 10., max: 30., mode: 25. }).unwrap());

    let reclaimer_2_mbox: Mailbox<VectorCombinerProcess> = Mailbox::new();
    let reclaimer_2_addr = reclaimer_2_mbox.address();

    let mut output_stockpile_2 = VectorStock::new()
        .with_name("Output Stockpile 2".into())
        .with_log_consumer(&stock_logger);
    output_stockpile_2.low_capacity = 100.;
    output_stockpile_2.max_capacity = 15000.;
    let output_stockpile_2_mbox: Mailbox<VectorStock> = Mailbox::new();
    let output_stockpile_2_addr = output_stockpile_2_mbox.address();

    let mut reclaimer_3 = VectorProcess::new()
        .with_name("Reclaimer 3".into());
    reclaimer_3.log_emitter.connect_sink(&logger.buffer);
    reclaimer_3.process_quantity_dist = Some(Distribution::Constant(100.));
    reclaimer_3.process_duration_secs_dist = Some(Distribution::Constant(60.));
    let reclaimer_3_mbox: Mailbox<VectorProcess> = Mailbox::new();
    let reclaimer_3_addr = reclaimer_3_mbox.address();

    let mut stacker = VectorSplitterProcess::new()
        .with_name("Stacker".into())
        .with_log_consumer(&logger);
    stacker.process_quantity_dist = Some(Distribution::Constant(600.));
    stacker.process_duration_secs_dist = Some(Distribution::Constant(360.));
    let stacker_mbox: Mailbox<VectorSplitterProcess> = Mailbox::new();
    let stacker_addr = stacker_mbox.address();

    // Connections

    // - Reclaimer 1:

    reclaimer_1
        .req_upstreams
        .0
        .connect(VectorStock::get_state, &stockpile_1_addr);
    reclaimer_1
        .withdraw_upstreams
        .0
        .connect(VectorStock::remove, &stockpile_1_addr);
    stockpile_1
        .state_emitter
        .connect(VectorCombinerProcess::check_update_state, &reclaimer_1_addr);

    reclaimer_1
        .req_upstreams
        .1
        .connect(VectorStock::get_state, &stockpile_2_addr);
    reclaimer_1
        .withdraw_upstreams
        .1
        .connect(VectorStock::remove, &stockpile_2_addr);
    stockpile_2
        .state_emitter
        .connect(VectorCombinerProcess::check_update_state, &reclaimer_1_addr);

    reclaimer_1
        .push_downstream
        .connect(VectorStock::add, &output_stockpile_1_addr);
    reclaimer_1
        .req_downstream
        .connect(VectorStock::get_state, &output_stockpile_1_addr);
    output_stockpile_1
        .state_emitter
        .connect(VectorCombinerProcess::check_update_state, &reclaimer_1_addr);

    // - Reclaimer 2:

    reclaimer_2
        .req_upstreams
        .0
        .connect(VectorStock::get_state, &stockpile_2_addr);
    reclaimer_2
        .withdraw_upstreams
        .0
        .connect(VectorStock::remove, &stockpile_2_addr);
    stockpile_2
        .state_emitter
        .connect(VectorCombinerProcess::check_update_state, &reclaimer_2_addr);

    reclaimer_2
        .req_upstreams
        .1
        .connect(VectorStock::get_state, &stockpile_3_addr);
    reclaimer_2
        .withdraw_upstreams
        .1
        .connect(VectorStock::remove, &stockpile_3_addr);
    stockpile_3
        .state_emitter
        .connect(VectorCombinerProcess::check_update_state, &reclaimer_2_addr);

    reclaimer_2
        .push_downstream
        .connect(VectorStock::add, &output_stockpile_2_addr);
    reclaimer_2
        .req_downstream
        .connect(VectorStock::get_state, &output_stockpile_2_addr);
    output_stockpile_2
        .state_emitter
        .connect(VectorCombinerProcess::check_update_state, &reclaimer_2_addr);

    // - Reclaimer 3

    reclaimer_3
        .req_upstream
        .connect(VectorStock::get_state, &stockpile_1_addr);
    reclaimer_3
        .withdraw_upstream
        .connect(VectorStock::remove, &stockpile_1_addr);
    stockpile_1
        .state_emitter
        .connect(VectorProcess::check_update_state, &reclaimer_3_addr);

    reclaimer_3
        .req_downstream
        .connect(VectorStock::get_state, &output_stockpile_1_addr);
    reclaimer_3
        .push_downstream
        .connect(VectorStock::add, &output_stockpile_1_addr);
    output_stockpile_1
        .state_emitter
        .connect(VectorProcess::check_update_state, &reclaimer_3_addr);

    // Stacker

    stacker
        .req_upstream
        .connect(VectorStock::get_state, &output_stockpile_1_addr);
    stacker
        .withdraw_upstream
        .connect(VectorStock::remove, &output_stockpile_1_addr);
    output_stockpile_1
        .state_emitter
        .connect(VectorSplitterProcess::check_update_state, &stacker_addr);

    stacker.req_downstreams.0.connect(VectorStock::get_state, &stockpile_2_addr);
    stacker.push_downstreams.0.connect(VectorStock::add, &stockpile_2_addr);
    stockpile_2.state_emitter.connect(VectorSplitterProcess::check_update_state, &stacker_addr);

    stacker.req_downstreams.1.connect(VectorStock::get_state, &stockpile_3_addr);
    stacker.push_downstreams.1.connect(VectorStock::add, &stockpile_3_addr);
    stockpile_3.state_emitter.connect(VectorSplitterProcess::check_update_state, &stacker_addr);

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
        .add_model(reclaimer_3, reclaimer_3_mbox, "Reclaimer 3")
        .add_model(stacker, stacker_mbox, "Stacker");

    let start_time = MonotonicTime::try_from_date_time(2025, 1, 1, 0, 0, 0, 0).unwrap();
    let mut simu = sim_builder.init(start_time).unwrap().0;
    simu.process_event(
        VectorCombinerProcess::check_update_state,
        NotificationMetadata {
            time: start_time,
            element_from: "Reclaimer 1".into(),
            message: "Start".into(),
        },
        &reclaimer_1_addr,
    ).unwrap();
    simu.process_event(
        VectorCombinerProcess::check_update_state,
        NotificationMetadata {
            time: start_time,
            element_from: "Reclaimer 2".into(),
            message: "Start".into(),
        },
        &reclaimer_2_addr,
    ).unwrap();
    simu.process_event(
        VectorProcess::check_update_state,
        NotificationMetadata {
            time: start_time,
            element_from: "Reclaimer 3".into(),
            message: "Start".into(),
        },
        &reclaimer_3_addr,
    ).unwrap();
    simu.process_event(
        VectorSplitterProcess::check_update_state,
        NotificationMetadata {
            time: start_time,
            element_from: "Stacker".into(),
            message: "Start".into(),
        },
        &stacker_addr,
    ).unwrap();

    simu.step_until(start_time + Duration::from_secs(60 * 60 * 1))
        .unwrap();

    std::fs::create_dir_all("outputs/material_blending").unwrap();
    logger.write_csv("outputs/material_blending".into()).unwrap();
    stock_logger.write_csv("outputs/material_blending".into()).unwrap();
}
