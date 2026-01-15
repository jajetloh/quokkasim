// Hybrid simulation POC

use quokkasim::prelude::*;
use std::{collections::HashMap, hash::Hash, ops::{Add, Deref, DerefMut, Mul}, sync::Arc, time::Duration};

struct PipeProcess {
    // Identification
    pub element_name: String,
    pub element_code: String,
    pub element_type: String,

    // Ports
    pub req_input_state: Requestor<(), ContStockState>,
    pub req_output_state: Requestor<(), ContStockState>,
    
    pub withdraw_input: Requestor<(f64, EventMetadata), f64>,
    pub push_output: Output<(f64, EventMetadata)>,
    pub log_emitter: Output<ContProcessLog<f64>>,

    pub rate_change_emitter: Output<(String, String, String, Arc<dyn Fn(f64, f64) -> f64 + Send + Sync>)>,
    pub rate_change_emitter_test: Output<(String, f64)>,

    pub req_integration: Requestor<EventMetadata, ()>,

    // Configuration
    pub transfer_rate_per_sec: f64,
    pub rate_function: Arc<dyn Fn(f64, f64) -> f64 + Send + Sync>,

    // Runtime state

    // Internals
    pub time_to_next_process_event: Option<Duration>,
    pub scheduled_event: Option<(MonotonicTime, ActionKey)>,
    pub previous_event: EventMetadata,
    pub last_integration_time: MonotonicTime,

    pub upstream_element_name: Option<String>,
    pub downstream_element_name: Option<String>,
}

impl Model for PipeProcess {
    fn init(
        self,
        _ctx: &mut Context<Self>,
    ) -> impl Future<Output = InitializedModel<Self>> {
        async move {
            self.into()
        }
    }
}

impl ContProcessCore for PipeProcess {
    fn element_name(&self) -> &str { &self.element_name }
    fn element_code(&self) -> &str { &self.element_code }
    fn element_type(&self) -> &str { &self.element_type }
    fn get_next_event_meta(&mut self) -> EventMetadata {
        self.previous_event.next()
    }
    fn scheduled_event(&mut self) -> &mut Option<(MonotonicTime, ActionKey)> {
        &mut self.scheduled_event
    }
    fn previous_check_time(&mut self) -> &mut MonotonicTime {
        &mut self.last_integration_time
    }
    fn time_to_next_process_event(&mut self) -> &mut Option<Duration> {
        &mut self.time_to_next_process_event
    }
    fn update_state(
        &mut self,
        source_event_id: EventMetadata,
        cx: &mut Context<Self>,
    ) -> impl Future<Output = ()> + Send {
        async move {
            // Implementation of state update logic goes here
            self.update_process_state_since_prev_event(&mut source_event_id.clone(), cx, Duration::from_secs(1)).await;
        }
    }
}

impl ContProcessUpdateSinceLast<f64> for PipeProcess {
    fn update_process_state_since_prev_event(
            &mut self, source_event_id: &mut EventMetadata,
            _cx: &mut Context<Self>,
            _duration_since_prev: Duration
        ) -> impl Future<Output = ()> {
        async move {
            // Implementation of process state update logic goes here
            self.req_integration.send(source_event_id.clone()).await.next();
        }
    }

    fn log_type_process_success(&mut self, source_event_id: &mut EventMetadata, quantity: f64, resource: f64, cx: &mut Context<Self>) -> impl Future<Output = EventMetadata> {
        async move {
            let new_event = self.previous_event.next();
            let log = ContProcessLog {
                time: cx.time().to_string(),
                event_id: new_event.clone(),
                source_event_id: source_event_id.clone(),
                element_name: self.element_name.clone(),
                element_type: "PipeProcess".to_string(),
                details: DefaultContProcessLogType::ProcessSuccess { quantity, resource }
            };
            self.log_emitter.send(log).await;
            new_event
        }
    }
}

impl PipeProcess {
    fn get_rate_function(&self) -> impl Future<Output = Arc<dyn Fn(f64, f64) -> f64 + Send + Sync + 'static>> {
        let rate_fn = Arc::clone(&self.rate_function);
        async move {
            rate_fn
        }
    }

    fn process_quantity(&mut self, payload: (f64, EventMetadata)) -> impl Future<Output = ()> {
        async move {
            let received = self.withdraw_input.send(payload.clone()).await.next().unwrap();
            self.push_output.send((received, payload.1)).await;
        }
    }
}

struct IntegrationService {
    // Identification
    pub element_name: String,
    pub element_code: String,
    pub element_type: String,

    // Ports
    pub log_emitter: Output<ContProcessLog<f64>>,
    pub req_stock_levels: HashMap<String, Requestor<(), ContStockState>>,
    pub push_process_quantities: HashMap<String, Output<(f64, EventMetadata)>>,

    // Configuration

    // Runtime state
    pub process_rate_functions: HashMap<
        String,
        (
            String, String, String,
            Arc<dyn Fn(f64, f64) -> f64 + Send + Sync + 'static>
        )
    >,

    // Internals
    pub time_to_next_process_event: Option<Duration>,
    pub scheduled_event: Option<(MonotonicTime, ActionKey)>,
    pub next_event_index: u64,
    pub last_integration_time: MonotonicTime,
}

impl Model for IntegrationService {
    fn init(
        mut self,
        ctx: &mut Context<Self>,
    ) -> impl Future<Output = InitializedModel<Self>> {
        async move {
            self.last_integration_time = ctx.time();
            self.into()
        }
    }
}

#[derive(Clone)]
struct VectorHashMap<K, V>(HashMap<K, V>);
impl<K, V> Deref for VectorHashMap<K, V> {
    type Target = HashMap<K, V>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<K, V> DerefMut for VectorHashMap<K, V> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl<K, V> Add for VectorHashMap<K, V> where K: Eq + Hash + Clone, V: Add<Output = V> + Clone + Default {
    type Output = VectorHashMap<K, V>;
    fn add(self, other: VectorHashMap<K, V>) -> VectorHashMap<K, V> {
        let mut result = self.0.clone();
        for (k, v) in other.0 {
            if result.contains_key(&k) {
                let entry = result.get_mut(&k).unwrap();
                *entry = entry.clone() + v;
            } else {
                result.insert(k, v);
            }
        }
        VectorHashMap(result)
    }
}

impl<K, V> Mul<f64> for VectorHashMap<K, V> where K: Eq + Hash + Clone, V: Mul<f64, Output = V> + Clone {
    type Output = VectorHashMap<K, V>;
    fn mul(self, scalar: f64) -> VectorHashMap<K, V> {
        let mut result = HashMap::new();
        for (k, v) in self.0 {
            result.insert(k, v * scalar);
        }
        VectorHashMap(result)
    }
}

impl IntegrationService {
    fn register_rate_function(&mut self, 
        params: (String, String, String, Arc<dyn Fn(f64, f64) -> f64 + Send + Sync>)
    ) {
        self.process_rate_functions.insert(params.0.clone(), (params.0, params.1, params.2, params.3));
    }

    fn register_rate_function_test(&mut self, _params: (String, f64)) {

    }

    fn integrate_rates(&mut self, source_event_id: EventMetadata, cx: &mut Context<Self>) -> impl Future<Output = ()> {
        async move {
            // Implementation of rate integration logic goes here

            let time_elapsed_secs = (cx.time().duration_since(self.last_integration_time)).as_secs_f64();

            let mut stock_values = HashMap::new();
            for (code, req) in self.req_stock_levels.iter_mut() {
                let value = req.send(()).await.next().unwrap();
                stock_values.insert(code.clone(), value);
            }
            let us_stock_state = stock_values.get("TANK1").unwrap();
            let ds_stock_state = stock_values.get("TANK2").unwrap();

            let init_state = VectorHashMap(HashMap::from([
                ("B0".to_string(), us_stock_state.occupied()),
                ("B1".to_string(), ds_stock_state.occupied()),
                ("F0".to_string(), 0.),
            ]));
            
            let dt_sec = time_elapsed_secs;
            let pipe_1_rate_fn = &self.process_rate_functions.get("PIPE1").unwrap().3;
            
            let diff_fn = |x: &VectorHashMap<String, f64>| -> VectorHashMap<String, f64> {
                let p01 = pipe_1_rate_fn(*x.get("B0").unwrap(), *x.get("B1").unwrap());
                VectorHashMap(HashMap::from([
                    ("B0".to_string(), -p01),
                    ("B1".to_string(), p01),
                    ("F0".to_string(), p01),
                ]))
            };
            let k1 = diff_fn(&init_state);
            let k2 = init_state.clone().add(k1.clone().mul(dt_sec / 2.0));
            let k3 = init_state.clone().add(k2.clone().mul(dt_sec / 2.0));
            let k4 = init_state.clone().add(k3.clone().mul(dt_sec));

            let state_after_dt = init_state.add(
                (k1.add(k2.mul(2.0)).add(k3.mul(2.0)).add(k4)).mul(dt_sec / 6.0)
            );

            self.push_process_quantities.get_mut("PIPE1").unwrap().send((
                state_after_dt.get("F0").unwrap().clone(),
                source_event_id.clone()
            )).await;


        }
    }
}

fn main() {
    // Component declarations
    let mut tank1 = DefaultContStock::<f64, ContStockState, ContStockLog<f64>>::new()
        .with_name("Tank 1")
        .with_code("TANK1")
        .with_low_capacity(0.)
        .with_max_capacity(1.)
        .with_initial_resource(1.);
    let tank1_mbox = Mailbox::new();
    let tank1_addr = tank1_mbox.address();

    let mut tank2 = DefaultContStock::<f64, ContStockState, ContStockLog<f64>>::new()
        .with_name("Tank 2")
        .with_code("TANK2")
        .with_low_capacity(0.)
        .with_max_capacity(1.)
        .with_initial_resource(0.);
    let tank2_mbox = Mailbox::new();
    let tank2_addr = tank2_mbox.address();

    let mut pipe1 = PipeProcess {
        element_name: "Pipe 1".into(),
        element_code: "PIPE1".into(),
        element_type: "PipeProcess".into(),
        req_input_state: Requestor::new(),
        req_output_state: Requestor::new(),
        req_integration: Requestor::new(),
        withdraw_input: Requestor::new(),
        push_output: Output::new(),
        log_emitter: Output::new(),
        rate_change_emitter: Output::new(),
        rate_change_emitter_test: Output::new(),
        transfer_rate_per_sec: 100.0,
        rate_function: Arc::new(|usl, dsl| {
            let n: f64 = 32.;
            let p: i32 = 1;
            let max_rate = 4.0;
            if usl <= 0. || dsl >= 1. {
                0.0
            } else if usl >= 1. || dsl <= 0. {
                max_rate
            } else {
                let x = n * (usl * (1. - dsl)).powi(p);
                let y = ((1.-usl) * dsl).powi(p);
                max_rate * x / (x + y)
            }
        }),
        time_to_next_process_event: None,
        scheduled_event: None,
        previous_event: EventMetadata::default(),
        last_integration_time: MonotonicTime::MIN,
        upstream_element_name: None,
        downstream_element_name: None,
    };
    let pipe1_mbox = Mailbox::new();
    let pipe1_addr = pipe1_mbox.address();

    let mut int_service = IntegrationService {
        element_name: "Integration Service".into(),
        element_code: "INTSVC".into(),
        element_type: "IntegrationService".into(),
        req_stock_levels: HashMap::new(),
        push_process_quantities: HashMap::new(),
        log_emitter: Output::new(),
        process_rate_functions: HashMap::new(),
        time_to_next_process_event: None,
        scheduled_event: None,
        next_event_index: 0,
        last_integration_time: MonotonicTime::MIN,
    };
    let int_service_mbox = Mailbox::new();
    let int_service_addr = int_service_mbox.address();

    // Connections

    pipe1.req_input_state.connect(DefaultContStock::get_state_async, tank1_addr.clone());
    pipe1.withdraw_input.connect(DefaultContStock::remove, tank1_addr.clone());
    pipe1.upstream_element_name = Some(tank1.element_name.clone());
    pipe1.req_output_state.connect(DefaultContStock::get_state_async, tank2_addr.clone());
    pipe1.push_output.connect(DefaultContStock::add, tank2_addr.clone());
    pipe1.downstream_element_name = Some(tank2.element_name.clone());

    pipe1.rate_change_emitter_test.connect(IntegrationService::register_rate_function_test, int_service_addr.clone());
    pipe1.rate_change_emitter.connect(IntegrationService::register_rate_function, int_service_addr.clone());
    pipe1.req_integration.connect(IntegrationService::integrate_rates, int_service_addr.clone());

    let mut r = Output::new();
    r.connect(PipeProcess::process_quantity, pipe1_addr.clone());
    int_service.push_process_quantities.insert("PIPE1".into(), r);

    let mut r = Requestor::new();
    r.connect(DefaultContStock::get_state_async, tank1_addr.clone());
    int_service.req_stock_levels.insert("TANK1".into(), r);

    let mut r = Requestor::new();
    r.connect(DefaultContStock::get_state_async, tank2_addr.clone());
    int_service.req_stock_levels.insert("TANK2".into(), r);

    int_service.register_rate_function((
        "PIPE1".into(),
        tank1.element_name.clone(),
        tank2.element_name.clone(),
        Arc::clone(&pipe1.rate_function),
    ));



    let stock_logger = EventQueue::<ContStockLog<f64>>::new();
    tank1.log_emitter.connect_sink(&stock_logger);
    tank2.log_emitter.connect_sink(&stock_logger);


    let sim_init = SimInit::new()
        .add_model(tank1, tank1_mbox, "Tank1")
        .add_model(tank2, tank2_mbox, "Tank2")
        .add_model(pipe1, pipe1_mbox, "Pipe1")
        .add_model(int_service, int_service_mbox, "IntegrationService");

    let mut model_time = MonotonicTime::EPOCH;

    let (mut sim, _sched) = sim_init.init(model_time).unwrap();

    let e = EventMetadata::from_scheduler();
    for _ in 0..400 {
        model_time += Duration::from_secs_f64(0.0005);
        sim.step_until(model_time).unwrap();
        sim.process_event(PipeProcess::update_state, e.clone(), pipe1_addr.clone()).unwrap();
    }

    for x in stock_logger.into_reader() {
        if x.element_name == "Tank 1"
            && let ContStockLogType::Remove { balance, resource: _ } = x.details {
                println!("{} | {}", x.time, balance);
            }
    }
    


}

