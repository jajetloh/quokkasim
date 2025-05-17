use std::{error::Error, time::Duration};

use nexosim::time::MonotonicTime;
use quokkasim::{define_model_enums, prelude::*};

define_model_enums! {
    pub enum ComponentModel<'a> {}
    pub enum ComponentLogger<'a> {}
}

impl<'a> CustomComponentConnection for ComponentModel<'a> {
    fn connect_components(a: Self, b: Self) -> Result<(), Box<dyn Error>> {
        match (a, b) {
            _ => Err("Invalid connection".into()),
        }
    }
}

impl<'a> CustomLoggerConnection<'a> for ComponentLogger<'a> {
    type ComponentType = ComponentModel<'a>;
    fn connect_logger(a: Self, b: Self::ComponentType) -> Result<(), Box<dyn Error>> {
        match (a, b) {
            _ => Err("Invalid connection".into()),
        }
    }
}


fn main() {
    // Declarations

    let process_logger = VectorProcessLogger::<f64>::new("ProcessLogger".into());
    let stock_logger = VectorStockLogger::<f64>::new("StockLogger".into());
    let mut df = DistributionFactory {
        base_seed: 1234,
        next_seed: 0,
    };

    let mut stockpile_1 = VectorStock::<Vector3>::new()
        .with_name("Stockpile 1".into())
        .with_low_capacity(100.)
        .with_max_capacity(10_000.)
        .with_initial_vector([8000., 2000., 0.].into());

    let stockpile_1_mbox: Mailbox<VectorStock<Vector3>> = Mailbox::new();
    let stockpile_1_addr = stockpile_1_mbox.address();

    let mut stockpile_2 = VectorStock::<Vector3>::new()
        .with_name("Stockpile 2".into())
        .with_low_capacity(100.)
        .with_max_capacity(10_000.)
        .with_initial_vector([0., 6000., 2000.].into());

    let stockpile_2_mbox: Mailbox<VectorStock<Vector3>> = Mailbox::new(); 
    let stockpile_2_addr = stockpile_2_mbox.address();

    let mut stockpile_3 = VectorStock::<Vector3>::new()
        .with_name("Stockpile 3".into())
        .with_low_capacity(100.)
        .with_max_capacity(10_000.)
        .with_initial_vector([5000., 5000., 0.].into());

    let stockpile_3_mbox: Mailbox<VectorStock<Vector3>> = Mailbox::new();
    let stockpile_3_addr = stockpile_3_mbox.address();

    let mut reclaimer_1: VectorCombiner<Vector3, Vector3, f64, 2> = VectorCombiner::new()
        .with_name("Reclaimer 1".into())
        .with_process_quantity_distr(Distribution::Constant(100.))
        .with_process_time_distr(Distribution::Constant(30.));

    let reclaimer_1_mbox = Mailbox::new();
    let reclaimer_1_addr = reclaimer_1_mbox.address();

    let mut output_stockpile_1 = VectorStock::<Vector3>::new()
        .with_name("Output Stockpile 1".into())
        .with_low_capacity(100.)
        .with_max_capacity(15_000.)
        .with_initial_vector([0., 0., 0.].into());

    let output_stockpile_1_mbox = Mailbox::new();
    let output_stockpile_1_addr = output_stockpile_1_mbox.address();

    let mut reclaimer_2: VectorCombiner<Vector3, Vector3, f64, 2> = VectorCombiner::new()
        .with_name("Reclaimer 2".into())
        .with_process_quantity_distr(Distribution::Constant(100.))
        .with_process_time_distr(Distribution::Constant(30.));

    let reclaimer_2_mbox= Mailbox::new();
    let reclaimer_2_addr = reclaimer_2_mbox.address();

    let mut output_stockpile_2 = VectorStock::<Vector3>::new()
        .with_name("Output Stockpile 2".into())
        .with_low_capacity(100.)
        .with_max_capacity(15_000.)
        .with_initial_vector([0., 0., 0.].into());
    let output_stockpile_2_mbox = Mailbox::new();
    let output_stockpile_2_addr = output_stockpile_2_mbox.address();

    let mut reclaimer_3 = VectorCombiner::<Vector3, f64, f64, 1>::new()
        .with_name("Reclaimer 3".into())
        .with_process_quantity_distr(Distribution::Constant(100.))
        .with_process_time_distr(Distribution::Constant(60.));
    let reclaimer_3_mbox = Mailbox::new();
    let reclaimer_3_addr = reclaimer_3_mbox.address();

    let mut stacker = VectorSplitter::<Vector3, Vector3, f64, 2>::new()
        .with_name("Stacker".into())
        .with_process_quantity_distr(Distribution::Constant(600.))
        .with_process_time_distr(Distribution::Constant(360.));
    let stacker_mbox = Mailbox::new();
    let stacker_addr = stacker_mbox.address();

    // Connections

    // - Reclaimer 1:

    ComponentModel::connect_components(
        ComponentModel::VectorStockVector3(&mut stockpile_1, &mut stockpile_1_addr),
        ComponentModel::VectorCom(&mut reclaimer_1, &mut reclaimer_1_addr),
    ).unwrap();

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
    process_logger.write_csv("outputs/material_blending".into()).unwrap();
    stock_logger.write_csv("outputs/material_blending".into()).unwrap();
}
