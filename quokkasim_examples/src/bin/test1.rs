use quokkasim::prelude::*;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use quokkasim::core::EventId;

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

impl VectorResource for IronOre {
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
    let mut s1: DefaultStock<_, VectorStockState> = DefaultStock::new(
        "TestStock1".to_string(),
        "TestStock1".to_string(),
        "TestStock1".to_string(),
        5.,
        100.,
        IronOre {
            fe: 50.,
            si: 40.,
            al: 5.,
            other: 5.,
            hematite: 10.,
            limonite: 5.,
            sericite: 2.,
        },
    );
    let s1_mbox = Mailbox::new();
    let s1_addr = s1_mbox.address();
    let mut p1: DefaultProcess<IronOre, VectorProcessLog<IronOre>> = DefaultProcess::new(
        "TestProcess1".to_string(),
        "TestProcess1".to_string(),
        "TestProcess1".to_string(),
        Distribution::Constant(1.),
        Distribution::Constant(2.0),
    );
    let p1_mbox = Mailbox::new();
    let p1_addr = p1_mbox.address();
    let mut s2: DefaultStock<_, VectorStockState> = DefaultStock::new(
        "TestStock2".to_string(),
        "TestStock2".to_string(),
        "TestStock2".to_string(),
        5.,
        100.,
        IronOre::default(),
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


struct MyCustomProcess<T: VectorResource + Clone + Send + Debug + Serialize + 'static> {
    process: DefaultProcess<T, VectorProcessLog<T>>,
    custom_field: f64,
}

impl<T: VectorResource + Clone + Send + Debug + Serialize + 'static> std::ops::Deref for MyCustomProcess<T> {
    type Target = DefaultProcess<T, VectorProcessLog<T>>;
    fn deref(&self) -> &Self::Target {
        &self.process
    }
}

impl<T: VectorResource + Clone + Send + Debug + Serialize + 'static> std::ops::DerefMut for MyCustomProcess<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.process
    }
}

impl<T: VectorResource + Clone + Send + Debug + Serialize + 'static> Process<VectorProcessLogType<T>> for MyCustomProcess<T> {
    fn log(&mut self, now: MonotonicTime, source_event_id: EventId, details: VectorProcessLogType<T>) -> impl Future<Output = EventId> {
        async move {
            println!("Here is some new stuff, hey {}!", self.custom_field);
            let new_event_id: EventId = EventId(format!("{}_{:06}", self.element_code, self.next_event_index));
            let log = VectorProcessLog::to_log(
                now,
                new_event_id.clone(),
                source_event_id,
                self.element_name.clone(),
                self.element_type.clone(),
                details,
            );
            self.log_emitter.send(log.clone()).await;
            self.next_event_index += 1;

            new_event_id
        }
    }
    fn update_state(&mut self, source_event_id: EventId, cx: &mut Context<Self>) -> impl Future<Output = ()> {
        async move {
            let inner_cx = unsafe {
                &mut *(cx as *mut Context<MyCustomProcess<T>> as *mut Context<DefaultProcess<T, VectorProcessLog<T>>>)
            };
            DefaultProcess::update_state(&mut self.process, source_event_id, inner_cx).await;
        }
    }
}

impl<T: VectorResource + Clone + Send + Debug + Serialize + 'static> Model for MyCustomProcess<T> {
    fn init(mut self, cx: &mut Context<Self>) -> impl Future<Output = InitializedModel<Self>> + Send {
        async move {
            self.update_state(EventId::from_init(), cx).await;
            self.into()
        }
    }
}

impl<
    T: VectorResource + Clone + Send + Debug + Serialize + 'static,
> Connect<MyCustomProcess<T>, DefaultStock<T, VectorStockState>> for Connection
    where MyCustomProcess<T>: Model,
          DefaultStock<T, VectorStockState>: Model
{
    fn connect(
        &mut self,
        a: (&mut MyCustomProcess<T>, &Address<MyCustomProcess<T>>),
        b: (&mut DefaultStock<T, VectorStockState>, &Address<DefaultStock<T, VectorStockState>>),
    ) -> Result<(), String> {
        a.0.push_downstream.connect(DefaultStock::add, b.1.clone());
        a.0.req_downstream.connect(DefaultStock::get_state_async, b.1.clone());
        b.0.state_emitter.connect(MyCustomProcess::update_state, a.1);
        Ok(())
    }
}

impl<
    T: VectorResource + Clone + Send + Debug + Serialize + Projectable<f64> + 'static,
> Connect<DefaultStock<T, VectorStockState>, MyCustomProcess<T>> for Connection
    where MyCustomProcess<T>: Model,
          DefaultStock<T, VectorStockState>: Model
{
    fn connect(
        &mut self,
        a: (&mut DefaultStock<T, VectorStockState>, &Address<DefaultStock<T, VectorStockState>>),
        b: (&mut MyCustomProcess<T>, &Address<MyCustomProcess<T>>),
    ) -> Result<(), String> {
        b.0.withdraw_upstream.connect(DefaultStock::remove, a.1.clone());
        b.0.req_upstream.connect(DefaultStock::get_state_async, a.1.clone());
        a.0.state_emitter.connect(MyCustomProcess::update_state, b.1);
        Ok(())
    }
}

fn bench_custom_process(_cfg: ()) -> Result<(Simulation, EndpointRegistry), SimulationError> {

    let mut s1: DefaultStock<_, VectorStockState> = DefaultStock::new(
        "TestStock1".to_string(),
        "TestStock1".to_string(),
        "TestStock1".to_string(),
        5.,
        100.,
        IronOre {
            fe: 50.,
            si: 40.,
            al: 5.,
            other: 5.,
            hematite: 10.,
            limonite: 5.,
            sericite: 2.,
        },
    );
    let s1_mbox = Mailbox::new();
    let s1_addr = s1_mbox.address();
    let mut p1: MyCustomProcess<IronOre> = MyCustomProcess {
        process: DefaultProcess::new(
            "TestProcess1".to_string(),
            "TestProcess1".to_string(),
            "TestProcess1".to_string(),
            Distribution::Constant(1.),
            Distribution::Constant(2.0),
        ),
        custom_field: 123.45,
    };
    let p1_mbox = Mailbox::new();
    let p1_addr = p1_mbox.address();
    let mut s2: DefaultStock<_, VectorStockState> = DefaultStock::new(
        "TestStock2".to_string(),
        "TestStock2".to_string(),
        "TestStock2".to_string(),
        5.,
        100.,
        IronOre::default(),
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
    server::run(bench_custom_process, "127.0.0.1:12345".parse().unwrap()).unwrap();
}