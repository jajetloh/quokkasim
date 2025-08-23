use quokkasim::prelude::*;

fn bench_f64(_cfg: ()) -> Result<(Simulation, EndpointRegistry), SimulationError> {
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
        Distribution::Constant(1.),
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


    // Connections

    let mut c = Connection {};
    c.connect((&mut s1, &s1_addr), (&mut p1, &p1_addr)).unwrap();
    c.connect((&mut p1, &p1_addr), (&mut s2, &s2_addr)).unwrap();
    
    // Loggers
    
    let process_logger = EventSlot::new();
    p1.log_emitter.connect_sink(&process_logger);

    // Registry

    let mut registry = EndpointRegistry::new();
    
    let mut input_add_to_s1 = EventSource::new();
    input_add_to_s1.connect(DefaultStock::add, &s1_addr);
    registry.add_event_source(input_add_to_s1, "add_to_s1").unwrap();

    let mut input_add_to_s2 = EventSource::new();
    input_add_to_s2.connect(DefaultStock::add, &s2_addr);
    registry.add_event_source(input_add_to_s2, "remove_from_s2").unwrap();

    let mut output = EventQueue::new();
    p1.log_emitter.connect_sink(&output);
    registry.add_event_sink(output.into_reader(), "process_log").unwrap();

    // Execution

    let mut sim_init = SimInit::new();
    sim_init = sim_init.add_model(s1, s1_mbox, "TestStock1");
    sim_init = sim_init.add_model(s2, s2_mbox, "TestStock2");
    sim_init = sim_init.add_model(p1, p1_mbox, "TestProcess1");
    let (mut simu, scheduler) = sim_init.init(MonotonicTime::EPOCH).unwrap();

    process_logger.into_iter().for_each(|log| {
        println!("Process Log: {:?}", log);
    });

    Ok((simu, registry))
}


fn bench_array_f64(_cfg: ()) -> Result<(Simulation, EndpointRegistry), SimulationError> {
    let mut s1: DefaultStock<[f64; 5], VectorStockState> = DefaultStock::new(
        "TestStock1".to_string(),
        "TestStock1".to_string(),
        "TestStock1".to_string(),
        5.,
        100.,
        [50., 40., 30., 20., 10.],
    );
    let s1_mbox = Mailbox::new();
    let s1_addr = s1_mbox.address();
    let mut p1: DefaultProcess<[f64; 5], VectorProcessLog<[f64; 5]>> = DefaultProcess::new(
        "TestProcess1".to_string(),
        "TestProcess1".to_string(),
        "TestProcess1".to_string(),
        Distribution::Constant(1.),
        Distribution::Constant(2.0),
    );
    let p1_mbox = Mailbox::new();
    let p1_addr = p1_mbox.address();
    let mut s2: DefaultStock<[f64; 5], VectorStockState> = DefaultStock::new(
        "TestStock2".to_string(),
        "TestStock2".to_string(),
        "TestStock2".to_string(),
        5.,
        100.,
        [0., 0., 0., 0., 0.],
    );
    let s2_mbox = Mailbox::new();
    let s2_addr = s2_mbox.address();


    // Connections

    let mut c = Connection {};
    c.connect((&mut s1, &s1_addr), (&mut p1, &p1_addr)).unwrap();
    c.connect((&mut p1, &p1_addr), (&mut s2, &s2_addr)).unwrap();
    
    // Loggers
    
    let process_logger = EventSlot::new();
    p1.log_emitter.connect_sink(&process_logger);

    // Registry

    let mut registry = EndpointRegistry::new();
    
    let mut input_add_to_s1 = EventSource::new();
    input_add_to_s1.connect(DefaultStock::add, &s1_addr);
    registry.add_event_source(input_add_to_s1, "add_to_s1").unwrap();

    let mut input_add_to_s2 = EventSource::new();
    input_add_to_s2.connect(DefaultStock::add, &s2_addr);
    registry.add_event_source(input_add_to_s2, "remove_from_s2").unwrap();

    let mut output = EventQueue::new();
    p1.log_emitter.connect_sink(&output);
    registry.add_event_sink(output.into_reader(), "process_log").unwrap();

    // Execution

    let mut sim_init = SimInit::new();
    sim_init = sim_init.add_model(s1, s1_mbox, "TestStock1");
    sim_init = sim_init.add_model(s2, s2_mbox, "TestStock2");
    sim_init = sim_init.add_model(p1, p1_mbox, "TestProcess1");
    let (mut simu, scheduler) = sim_init.init(MonotonicTime::EPOCH).unwrap();

    process_logger.into_iter().for_each(|log| {
        println!("Process Log: {:?}", log);
    });

    Ok((simu, registry))
}

fn main() {
    server::run(bench_array_f64, "127.0.0.1:12345".parse().unwrap()).unwrap();
}