use quokkasim::prelude::*;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

fn bench_f64(_cfg: ()) -> Result<(Simulation, EndpointRegistry), SimulationError> {
    let mut s1: DefaultContStock<f64, ContStockState, ContStockLog<f64>> = DefaultContStock::new()
        .with_name("TestStock1")
        .with_code("S1")
        .with_low_capacity(5.)
        .with_max_capacity(100.)
        .with_initial_resource(100.);
    let s1_mbox: Mailbox<DefaultContStock<f64, ContStockState, ContStockLog<f64>>> = Mailbox::new();
    let s1_addr: Address<DefaultContStock<f64, ContStockState, ContStockLog<f64>>> = s1_mbox.address();
    let mut p1: DefaultContProcess<f64, ContProcessLog<DefaultContProcessLogType<f64>, f64>> = DefaultContProcess::new()
        .with_name("TestProcess1")
        .with_code("P1")
        .with_type("ProcessType1")
        .with_process_quantity_distr(Distribution::Constant(1.))
        .with_process_time_distr(Distribution::Constant(2.0));
    let p1_mbox = Mailbox::new();
    let p1_addr = p1_mbox.address();
    let mut s2: DefaultContStock<f64, ContStockState, ContStockLog<f64>> = DefaultContStock::new()
        .with_name("TestStock2")
        .with_code("S2")
        .with_low_capacity(5.)
        .with_max_capacity(100.)
        .with_initial_resource(0.);
    let s2_mbox: Mailbox<DefaultContStock<f64, ContStockState, ContStockLog<f64>>> = Mailbox::new();
    let s2_addr: Address<DefaultContStock<f64, ContStockState, ContStockLog<f64>>> = s2_mbox.address();

    // Connections

    let mut c = Connection {};
    c.connect((&mut s1, &s1_addr), (&mut p1, &p1_addr)).unwrap();
    c.connect((&mut p1, &p1_addr), (&mut s2, &s2_addr)).unwrap();
    
    // Loggers
    
    let process_logger = EventSlot::new();
    p1.log_emitter.connect_sink(&process_logger);

    // Registry

    let mut registry = EndpointRegistry::new();
    
    let mut input_add_to_s1: EventSource<(f64, EventId)> = EventSource::new();
    input_add_to_s1.connect(DefaultContStock::add, &s1_addr);
    registry.add_event_source(input_add_to_s1, "add_to_s1").unwrap();

    let mut input_add_to_s2: EventSource<(f64, EventId)> = EventSource::new();
    input_add_to_s2.connect(DefaultContStock::add, &s2_addr);
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
    let mut s1: DefaultContStock<[f64; 5], ContStockState, ContStockLog<[f64; 5]>> = DefaultContStock::new()
        .with_name("TestStock1")
        .with_code("S1")
        .with_low_capacity(5.)
        .with_max_capacity(100.)
        .with_initial_resource([50., 40., 30., 20., 10.]);
    let s1_mbox = Mailbox::new();
    let s1_addr = s1_mbox.address();
    let mut p1: DefaultContProcess<[f64; 5], ContProcessLog<DefaultContProcessLogType<[f64; 5]>, [f64; 5]>> = DefaultContProcess::new()
        .with_name("TestProcess1")
        .with_code("P1")
        .with_type("ProcessType1")
        .with_process_quantity_distr(Distribution::Constant(1.))
        .with_process_time_distr(Distribution::Constant(2.0));
    let p1_mbox = Mailbox::new();
    let p1_addr = p1_mbox.address();
    let mut s2: DefaultContStock<[f64; 5], ContStockState, ContStockLog<[f64; 5]>> = DefaultContStock::new()
        .with_name("TestStock2")
        .with_code("S2")
        .with_low_capacity(5.)
        .with_max_capacity(100.)
        .with_initial_resource([0., 0., 0., 0., 0.]);
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
    input_add_to_s1.connect(DefaultContStock::add, &s1_addr);
    registry.add_event_source(input_add_to_s1, "add_to_s1").unwrap();

    let mut input_add_to_s2 = EventSource::new();
    input_add_to_s2.connect(DefaultContStock::add, &s2_addr);
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

#[derive(Clone, Debug, Serialize, Deserialize)]
struct IronOre {
    fe: f64,
    si: f64,
    al: f64,
    other: f64,
    hematite: f64,
    limonite: f64,
    sericite: f64,
}

impl Projectable<f64> for IronOre {
    fn project(self, arg: f64) -> Self {
        let total = self.total();
        IronOre {
            fe: self.fe / total * arg,
            si: self.si / total * arg,
            al: self.al / total * arg,
            other: self.other / total * arg,
            hematite: self.hematite / total * arg,
            limonite: self.limonite / total * arg,
            sericite: self.sericite / total * arg,
        }
    }
}

impl ContArithmetic for IronOre {
    fn total(&self) -> f64 {
        self.fe + self.si + self.al + self.other
    }
    fn add(&mut self, arg: Self) {
        self.fe += arg.fe;
        self.si += arg.si;
        self.al += arg.al;
        self.other += arg.other;
        self.hematite += arg.hematite;
        self.limonite += arg.limonite;
        self.sericite += arg.sericite;
    }
    fn remove<T>(&mut self, arg: T) -> Self where Self: Projectable<T> {
        let removed = self.clone().project(arg);
        self.fe -= removed.fe;
        self.si -= removed.si;
        self.al -= removed.al;
        self.other -= removed.other;
        self.hematite -= removed.hematite;
        self.limonite -= removed.limonite;
        self.sericite -= removed.sericite;
        removed
    }
    fn multiply(&mut self, arg: f64) {
        self.fe *= arg;
        self.si *= arg;
        self.al *= arg;
        self.other *= arg;
        self.hematite *= arg;
        self.limonite *= arg;
        self.sericite *= arg;
    }
    fn remove_all(&mut self) -> Self {
        let removed = self.clone();
        self.fe = 0.0;
        self.si = 0.0;
        self.al = 0.0;
        self.other = 0.0;
        self.hematite = 0.0;
        self.limonite = 0.0;
        self.sericite = 0.0;
        removed
    }
}

impl Default for IronOre {
    fn default() -> Self {
        IronOre {
            fe: 0.0,
            si: 0.0,
            al: 0.0,
            other: 0.0,
            hematite: 0.0,
            limonite: 0.0,
            sericite: 0.0,
        }
    }
}

fn bench_custom_resource(_cfg: ()) -> Result<(Simulation, EndpointRegistry), SimulationError> {
    let mut df = DistributionFactory::new(12345);
    let mut s1: DefaultContStock<_, ContStockState, ContStockLog<_>> = DefaultContStock::new()
        .with_name("TestStock1")
        .with_code("S1")
        .with_low_capacity(5.)
        .with_max_capacity(100.)
        .with_initial_resource(IronOre {
            fe: 90.,
            si: 50.,
            al: 5.,
            other: 5.,
            hematite: 10.,
            limonite: 5.,
            sericite: 2.,
        });
    let s1_mbox = Mailbox::new();
    let s1_addr = s1_mbox.address();

    let mut p1: DefaultContProcess<IronOre, ContProcessLog<DefaultContProcessLogType<IronOre>, IronOre>> = DefaultContProcess::new()
        .with_name("TestProcess1")
        .with_code("P1")
        .with_type("ProcessType1")
        .with_process_quantity_distr(Distribution::Constant(1.))
        .with_process_time_distr(df.create(DistributionConfig::TruncNormal { mean: 3600., std: 1200., min: Some(1.), max: None }).unwrap());
    p1.delay_modes.modify(DelayModeChange::Add(DelayMode {
        name: "TestDelay".to_string(),
        until_delay_distr: df.create(DistributionConfig::Exponential { mean: 7200. }).unwrap(),
        until_fix_distr: df.create(DistributionConfig::Uniform { min: 1000., max: 2000. }).unwrap()
    }));
    let p1_mbox = Mailbox::new();
    let p1_addr = p1_mbox.address();

    let mut s2: DefaultContStock<_, ContStockState, ContStockLog<_>> = DefaultContStock::new()
        .with_name("TestStock2")
        .with_code("S2")
        .with_low_capacity(5.)
        .with_max_capacity(100.)
        .with_initial_resource(IronOre::default());
    let s2_mbox = Mailbox::new();
    let s2_addr = s2_mbox.address();

    let mut p2: DefaultContProcess<IronOre, ContProcessLog<DefaultContProcessLogType<IronOre>, IronOre>> = DefaultContProcess::new()
        .with_name("TestProcess2")
        .with_code("P2")
        .with_type("ProcessType2")
        .with_process_quantity_distr(df.create(DistributionConfig::Triangular { min: 0.6, max: 2.0, mode: 0.6 }).unwrap())
        .with_process_time_distr(df.create(DistributionConfig::TruncNormal { mean: 3600., std: 1200., min: Some(1.), max: None }).unwrap());
    p2.delay_modes.modify(DelayModeChange::Add(DelayMode {
        name: "TestDelay".to_string(),
        until_delay_distr: df.create(DistributionConfig::Exponential { mean: 7200. }).unwrap(),
        until_fix_distr: df.create(DistributionConfig::Uniform { min: 1000., max: 2000. }).unwrap()
    }));
    let p2_mbox = Mailbox::new();
    let p2_addr = p2_mbox.address();

    let mut s3: DefaultContStock<_, ContStockState, ContStockLog<_>> = DefaultContStock::new()
        .with_name("TestStock3")
        .with_code("S3")
        .with_low_capacity(5.)
        .with_max_capacity(100.)
        .with_initial_resource(IronOre::default());
    let s3_mbox = Mailbox::new();
    let s3_addr = s3_mbox.address();

    let mut p3: DefaultContProcess<IronOre, ContProcessLog<DefaultContProcessLogType<IronOre>, IronOre>> = DefaultContProcess::new()
        .with_name("TestProcess3")
        .with_code("P3")
        .with_type("ProcessType3")
        .with_process_quantity_distr(Distribution::Constant(1.))
        .with_process_time_distr(df.create(DistributionConfig::Exponential { mean: 3600. }).unwrap());
    p3.delay_modes.modify(DelayModeChange::Add(DelayMode {
        name: "TestDelay".to_string(),
        until_delay_distr: df.create(DistributionConfig::Exponential { mean: 7200. }).unwrap(),
        until_fix_distr: df.create(DistributionConfig::Uniform { min: 1000., max: 2000. }).unwrap()
    }));
    let p3_mbox = Mailbox::new();
    let p3_addr = p3_mbox.address();

    let mut p4: DefaultContProcess<IronOre, ContProcessLog<DefaultContProcessLogType<IronOre>, IronOre>> = DefaultContProcess::new()
        .with_name("TestProcess4")
        .with_code("P4")
        .with_type("ProcessType4")
        .with_process_quantity_distr(df.create(DistributionConfig::TruncNormal { mean: 1., std: 0.3, min: Some(0.1), max: None }).unwrap())
        .with_process_time_distr(df.create(DistributionConfig::Exponential { mean: 3600. }).unwrap());
    // p4.delay_modes.modify(DelayModeChange::Add(DelayMode {
    //     name: "TestDelay".to_string(),
    //     until_delay_distr: Distribution::Constant(5.1),
    //     until_fix_distr: Distribution::Constant(0.2),
    // }));
    let p4_mbox = Mailbox::new();
    let p4_addr = p4_mbox.address();

    let mut s4: DefaultContStock<_, ContStockState, ContStockLog<_>> = DefaultContStock::new()
        .with_name("TestStock4")
        .with_code("S4")
        .with_low_capacity(5.)
        .with_max_capacity(10000.)
        .with_initial_resource(IronOre::default());
    let s4_mbox = Mailbox::new();
    let s4_addr = s4_mbox.address();


    // Connections

    let mut c = Connection {};
    c.connect((&mut s1, &s1_addr), (&mut p1, &p1_addr)).unwrap();
    c.connect((&mut p1, &p1_addr), (&mut s2, &s2_addr)).unwrap();
    c.connect((&mut s2, &s2_addr), (&mut p2, &p2_addr)).unwrap();
    c.connect((&mut p2, &p2_addr), (&mut s3, &s3_addr)).unwrap();
    c.connect((&mut s3, &s3_addr), (&mut p3, &p3_addr)).unwrap();
    c.connect((&mut p3, &p3_addr), (&mut s1, &s1_addr)).unwrap();

    c.connect((&mut s3, &s3_addr), (&mut p4, &p4_addr)).unwrap();
    c.connect((&mut p4, &p4_addr), (&mut s4, &s4_addr)).unwrap();
    // Loggers
    
    let process_logger = EventSlot::new();
    p1.log_emitter.connect_sink(&process_logger);

    // Registry

    let mut registry = EndpointRegistry::new();
    
    let mut input_add_to_s1 = EventSource::new();
    input_add_to_s1.connect(DefaultContStock::add, &s1_addr);
    registry.add_event_source(input_add_to_s1, "add_to_s1").unwrap();

    let mut input_remove_from_s2 = EventSource::new();
    input_remove_from_s2.connect(DefaultContStock::remove_void, &s2_addr);
    registry.add_event_source(input_remove_from_s2, "remove_from_s2").unwrap();

    let output_process_log = EventQueue::new();
    p1.log_emitter.connect_sink(&output_process_log);
    p2.log_emitter.connect_sink(&output_process_log);
    p3.log_emitter.connect_sink(&output_process_log);
    p4.log_emitter.connect_sink(&output_process_log);
    registry.add_event_sink(output_process_log.into_reader(), "process_log").unwrap();

    let output_stock_log = EventQueue::new();
    s1.log_emitter.connect_sink(&output_stock_log);
    s2.log_emitter.connect_sink(&output_stock_log);
    s3.log_emitter.connect_sink(&output_stock_log);
    s4.log_emitter.connect_sink(&output_stock_log);
    registry.add_event_sink(output_stock_log.into_reader(), "stock_log").unwrap();

    // Execution

    let mut sim_init = SimInit::new();
    sim_init = sim_init.add_model(s1, s1_mbox, "TestStock1");
    sim_init = sim_init.add_model(s2, s2_mbox, "TestStock2");
    sim_init = sim_init.add_model(s3, s3_mbox, "TestStock3");
    sim_init = sim_init.add_model(s4, s4_mbox, "TestStock4");
    sim_init = sim_init.add_model(p1, p1_mbox, "TestProcess1");
    sim_init = sim_init.add_model(p2, p2_mbox, "TestProcess2");
    sim_init = sim_init.add_model(p3, p3_mbox, "TestProcess3");
    sim_init = sim_init.add_model(p4, p4_mbox, "TestProcess4");

    let (mut simu, scheduler) = sim_init.init(MonotonicTime::EPOCH).unwrap();

    Ok((simu, registry))
}

fn bench_f64_resource_v2(_cfg: ()) -> Result<(Simulation, EndpointRegistry), SimulationError> {
    let mut df = DistributionFactory::new(12345);
    let mut s1: DefaultContStock<_, ContStockState, ContStockLog<_>> = DefaultContStock::new()
        .with_name("TestStock1")
        .with_code("S1")
        .with_low_capacity(5.)
        .with_max_capacity(100.)
        .with_initial_resource(150.);
    let s1_mbox = Mailbox::new();
    let s1_addr = s1_mbox.address();

    let mut p1: DefaultContProcess<f64, ContProcessLog<DefaultContProcessLogType<f64>, f64>> = DefaultContProcess::new()
        .with_name("TestProcess1")
        .with_code("P1")
        .with_type("ProcessType1")
        .with_process_quantity_distr(df.create(DistributionConfig::Triangular { min: 0.6, max: 2.0, mode: 1.2 }).unwrap())
        .with_process_time_distr(df.create(DistributionConfig::TruncNormal { mean: 3600., std: 1200., min: Some(1.), max: None }).unwrap());
    p1.delay_modes.modify(DelayModeChange::Add(DelayMode {
        name: "TestDelay".to_string(),
        until_delay_distr: df.create(DistributionConfig::Exponential { mean: 7200. }).unwrap(),
        until_fix_distr: df.create(DistributionConfig::Uniform { min: 1000., max: 2000. }).unwrap()
    }));
    let p1_mbox = Mailbox::new();
    let p1_addr = p1_mbox.address();

    let mut s2: DefaultContStock<_, ContStockState, ContStockLog<_>> = DefaultContStock::new()
        .with_name("TestStock2")
        .with_code("S2")
        .with_low_capacity(5.)
        .with_max_capacity(100.);
    let s2_mbox = Mailbox::new();
    let s2_addr = s2_mbox.address();

    let mut p2: DefaultContProcess<f64, ContProcessLog<DefaultContProcessLogType<f64>, f64>> = DefaultContProcess::new()
        .with_name("TestProcess2")
        .with_code("P2")
        .with_type("ProcessType2")
        .with_process_quantity_distr(df.create(DistributionConfig::Triangular { min: 0.6, max: 2.0, mode: 0.6 }).unwrap())
        .with_process_time_distr(df.create(DistributionConfig::TruncNormal { mean: 3600., std: 1200., min: Some(1.), max: None }).unwrap());
    p2.delay_modes.modify(DelayModeChange::Add(DelayMode {
        name: "TestDelay".to_string(),
        until_delay_distr: df.create(DistributionConfig::Exponential { mean: 7200. }).unwrap(),
        until_fix_distr: df.create(DistributionConfig::Uniform { min: 1000., max: 2000. }).unwrap()
    }));
    let p2_mbox = Mailbox::new();
    let p2_addr = p2_mbox.address();

    let mut s3: DefaultContStock<_, ContStockState, ContStockLog<_>> = DefaultContStock::new()
        .with_name("TestStock3")
        .with_code("S3")
        .with_low_capacity(5.)
        .with_max_capacity(100.);
    let s3_mbox = Mailbox::new();
    let s3_addr = s3_mbox.address();

    let mut p3: DefaultContProcess<f64, ContProcessLog<DefaultContProcessLogType<f64>, f64>> = DefaultContProcess::new()
        .with_name("TestProcess3")
        .with_code("P3")
        .with_type("ProcessType3")
        .with_process_quantity_distr(Distribution::Constant(1.))
        .with_process_time_distr(df.create(DistributionConfig::Exponential { mean: 3600. }).unwrap());
    p3.delay_modes.modify(DelayModeChange::Add(DelayMode {
        name: "TestDelay".to_string(),
        until_delay_distr: df.create(DistributionConfig::Exponential { mean: 7200. }).unwrap(),
        until_fix_distr: df.create(DistributionConfig::Uniform { min: 1000., max: 2000. }).unwrap()
    }));
    let p3_mbox = Mailbox::new();
    let p3_addr = p3_mbox.address();

    let mut p4: DefaultContProcess<f64, ContProcessLog<DefaultContProcessLogType<f64>, f64>> = DefaultContProcess::new()
        .with_name("TestProcess4")
        .with_code("P4")
        .with_type("ProcessType4")
        .with_process_quantity_distr(df.create(DistributionConfig::TruncNormal { mean: 0.4, std: 0.3, min: Some(0.1), max: None }).unwrap())
        .with_process_time_distr(df.create(DistributionConfig::Exponential { mean: 3600. }).unwrap());
    let p4_mbox = Mailbox::new();
    let p4_addr = p4_mbox.address();

    let mut s4: DefaultContStock<_, ContStockState, ContStockLog<_>> = DefaultContStock::new()
        .with_name("TestStock4")
        .with_code("S4")
        .with_low_capacity(5.)
        .with_max_capacity(10000.);
    let s4_mbox = Mailbox::new();
    let s4_addr = s4_mbox.address();

    // Connections

    let mut c = Connection {};
    c.connect((&mut s1, &s1_addr), (&mut p1, &p1_addr)).unwrap();
    c.connect((&mut p1, &p1_addr), (&mut s2, &s2_addr)).unwrap();
    c.connect((&mut s2, &s2_addr), (&mut p2, &p2_addr)).unwrap();
    c.connect((&mut p2, &p2_addr), (&mut s3, &s3_addr)).unwrap();
    c.connect((&mut s3, &s3_addr), (&mut p3, &p3_addr)).unwrap();
    c.connect((&mut p3, &p3_addr), (&mut s1, &s1_addr)).unwrap();

    c.connect((&mut s3, &s3_addr), (&mut p4, &p4_addr)).unwrap();
    c.connect((&mut p4, &p4_addr), (&mut s4, &s4_addr)).unwrap();

    // Loggers
    
    let process_logger = EventSlot::new();
    p1.log_emitter.connect_sink(&process_logger);

    // Registry

    let mut registry = EndpointRegistry::new();
    
    let mut input_add_to_s1 = EventSource::new();
    input_add_to_s1.connect(DefaultContStock::add, &s1_addr);
    registry.add_event_source(input_add_to_s1, "add_to_s1").unwrap();

    let mut input_remove_from_s2 = EventSource::new();
    input_remove_from_s2.connect(DefaultContStock::remove_void, &s2_addr);
    registry.add_event_source(input_remove_from_s2, "remove_from_s2").unwrap();

    let output_process_log = EventQueue::new();
    p1.log_emitter.connect_sink(&output_process_log);
    p2.log_emitter.connect_sink(&output_process_log);
    p3.log_emitter.connect_sink(&output_process_log);
    p4.log_emitter.connect_sink(&output_process_log);
    registry.add_event_sink(output_process_log.into_reader(), "process_log").unwrap();

    let output_stock_log = EventQueue::new();
    s1.log_emitter.connect_sink(&output_stock_log);
    s2.log_emitter.connect_sink(&output_stock_log);
    s3.log_emitter.connect_sink(&output_stock_log);
    s4.log_emitter.connect_sink(&output_stock_log);
    registry.add_event_sink(output_stock_log.into_reader(), "stock_log").unwrap();

    // Execution

    let mut sim_init = SimInit::new();
    sim_init = sim_init.add_model(s1, s1_mbox, "TestStock1");
    sim_init = sim_init.add_model(s2, s2_mbox, "TestStock2");
    sim_init = sim_init.add_model(s3, s3_mbox, "TestStock3");
    sim_init = sim_init.add_model(s4, s4_mbox, "TestStock4");
    sim_init = sim_init.add_model(p1, p1_mbox, "TestProcess1");
    sim_init = sim_init.add_model(p2, p2_mbox, "TestProcess2");
    sim_init = sim_init.add_model(p3, p3_mbox, "TestProcess3");
    sim_init = sim_init.add_model(p4, p4_mbox, "TestProcess4");

    let (mut simu, scheduler) = sim_init.init(MonotonicTime::EPOCH).unwrap();

    Ok((simu, registry))
}

fn main() {
    server::run(bench_f64_resource_v2, "127.0.0.1:12345".parse().unwrap()).unwrap();
}