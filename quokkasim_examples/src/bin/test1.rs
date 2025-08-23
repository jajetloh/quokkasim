use quokkasim::prelude::*;
use std::time::Duration;

fn main() {
    println!("This is a placeholder for the main function in quokkasim_examples/src/test1.rs");

    let mut s1: DefaultStock<f64, VectorStockState> = DefaultStock::new(
        "TestStock1".to_string(),
        "TestStock1".to_string(),
        "TestStock1".to_string(),
        5.,
        100.,
        100.,
    );
    let s1_mbox = Mailbox::new();
    let s1_addr = s1_mbox.address();
    let mut p1: DefaultProcess<f64, VectorProcessLog<f64>> = DefaultProcess::new(
        "TestProcess1".to_string(),
        "TestProcess1".to_string(),
        "TestProcess1".to_string(),
        Distribution::Constant(10.0),
        Distribution::Constant(2.0),
    );
    let p1_mbox = Mailbox::new();
    let p1_addr = p1_mbox.address();
    let mut s2: DefaultStock<f64, VectorStockState> = DefaultStock::new(
        "TestStock2".to_string(),
        "TestStock2".to_string(),
        "TestStock2".to_string(),
        5.,
        100.,
        0.,
    );
    let s2_mbox = Mailbox::new();
    let s2_addr = s2_mbox.address();

    let mut c = Connection {};
    c.connect((&mut p1, p1_addr), (&mut s2, s2_addr)).unwrap();
    let process_logger = EventSlot::new();
    p1.log_emitter.connect_sink(&process_logger);

    let mut sim_init = SimInit::new();
    sim_init = sim_init.add_model(s1, s1_mbox, "TestStock1");
    sim_init = sim_init.add_model(s2, s2_mbox, "TestStock2");
    sim_init = sim_init.add_model(p1, p1_mbox, "TestProcess1");
    let (mut simu, scheduler) = sim_init.init(MonotonicTime::EPOCH).unwrap();
    simu.step_until(MonotonicTime::EPOCH + Duration::from_secs(60)).unwrap();

    process_logger.into_iter().for_each(|log| {
        println!("Process Log: {:?}", log);
    });
    
}