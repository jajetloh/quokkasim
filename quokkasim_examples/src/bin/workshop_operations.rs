/*
 * Simulation of staff operations in a workshop, including parallel workstations
 * and staffing constraints.
 */

use quokkasim::prelude::*;
use serde::Serialize;
use std::{time::{Duration, SystemTime}};

#[derive(Clone, Serialize, Debug)]
struct Worker(String); // Simple wrapper type for worker resource
impl Worker {
    fn new(name: &str) -> Worker {
        Worker(name.to_string())
    }
}

#[derive(Clone, Serialize, Debug)]
struct Job(String); // Simple wrapper type for job resource

struct WorkerShiftManager {
    // Identification
    pub element_name: String,
    pub element_code: String,
    pub element_type: String,

    // Ports
    pub add_worker: Output<(Worker, EventId)>,
    pub remove_worker: Requestor<(Worker, EventId)>,
    pub log_emitter: Output<DiscProcessLog<DefaultDiscProcessLogType<Worker>>>,

    // Configuration
    pub weekly_work_schedule: Vec<(Worker, Duration, Duration)>,

    // Runtime state
    pub current_workers: Vec<(Duration, Worker)>,

    // Internals
    pub time_to_next_process_event: Option<Duration>,
    pub scheduled_event: Option<(MonotonicTime, ActionKey)>,
    pub next_event_index: u64,
    pub previous_check_time: MonotonicTime,
}

impl Model for WorkerShiftManager {
    fn init(
        mut self,
        ctx: &mut Context<Self>,
    ) -> impl Future<Output = InitializedModel<Self>> {
        async move {
            let source_event_id = EventId(format!(
                "{}_{:06}",
                self.element_code, self.next_event_index
            ));
            self.update_state(source_event_id, ctx).await;
            self.into()
        }
    }
}

impl DiscProcessCore<Worker, DiscProcessLog<DefaultDiscProcessLogType<Worker>>> for WorkerShiftManager {
    fn update_state(
            &mut self,
            source_event_id: EventId,
            cx: &mut Context<Self>,
        ) -> impl Future<Output = ()> + Send {
        async move {
            // self.up
        }
    }
    fn element_name(&self) -> &str { &self.element_name }
    fn element_code(&self) -> &str { &self.element_code }
    fn element_type(&self) -> &str { &self.element_type }
    fn get_next_event_id(&mut self) -> EventId {
        let id = EventId(format!(
            "{}_{:06}",
            self.element_code, self.next_event_index
        ));
        self.next_event_index += 1;
        id
    }
    fn log_emitter(&mut self) -> &mut Output<DiscProcessLog<DefaultDiscProcessLogType<Worker>>> { &mut self.log_emitter }
    fn scheduled_event(&mut self) -> &mut Option<(MonotonicTime, ActionKey)> {
        &mut self.scheduled_event
    }
    fn time_to_next_process_event(&mut self) -> &mut Option<Duration> {
        &mut self.time_to_next_process_event
    }
    fn previous_check_time(&mut self) -> &mut MonotonicTime {
        &mut self.previous_check_time
    }
    fn log_type_process_continue(&mut self, source_event_id: &mut EventId, reason: &'static str, cx: &mut Context<Self>) -> impl Future<Output = EventId> {
        async move {
            let current_event_id = self.get_next_event_id();
            self.log_emitter.send(DiscProcessLog {
                time: cx.time().to_chrono_date_time(0).unwrap().to_string(),
                event_id: current_event_id.clone(),
                source_event_id: source_event_id.clone(),
                element_name: self.element_name.clone(),
                element_type: self.element_type.clone(),
                details: DefaultDiscProcessLogType::ProcessContinue { reason }
            }).await;
            current_event_id
        }
    }
    fn log_type_withdraw_request(&mut self, source_event_id: &mut EventId, quantity: usize, cx: &mut Context<Self>) -> impl Future<Output = EventId> {
        async move {
            let current_event_id = self.get_next_event_id();
            self.log_emitter.send(DiscProcessLog {
                time: cx.time().to_chrono_date_time(0).unwrap().to_string(),
                event_id: current_event_id.clone(),
                source_event_id: source_event_id.clone(),
                element_name: self.element_name.clone(),
                element_type: self.element_type.clone(),
                details: DefaultDiscProcessLogType::WithdrawRequest { quantity }
            }).await;
            current_event_id
        }
    }
    fn log_type_process_success(&mut self, source_event_id: &mut EventId, quantity: usize, resources: Vec<Worker>, cx: &mut Context<Self>) -> impl Future<Output = EventId> {
        async move {
            let current_event_id = self.get_next_event_id();
            self.log_emitter.send(DiscProcessLog {
                time: cx.time().to_chrono_date_time(0).unwrap().to_string(),
                event_id: current_event_id.clone(),
                source_event_id: source_event_id.clone(),
                element_name: self.element_name.clone(),
                element_type: self.element_type.clone(),
                details: DefaultDiscProcessLogType::ProcessSuccess { quantity, resources }
            }).await;
            current_event_id
        }
    }
    fn log_type_process_start(&mut self, source_event_id: &mut EventId, quantity: usize, resources: Vec<Worker>, cx: &mut Context<Self>) -> impl Future<Output = EventId> {
        async move {
            let current_event_id = self.get_next_event_id();
            self.log_emitter.send(DiscProcessLog {
                time: cx.time().to_chrono_date_time(0).unwrap().to_string(),
                event_id: current_event_id.clone(),
                source_event_id: source_event_id.clone(),
                element_name: self.element_name.clone(),
                element_type: self.element_type.clone(),
                details: DefaultDiscProcessLogType::ProcessStart { quantity, resources }
            }).await;
            current_event_id
        }
    }
    fn log_type_process_failure(&mut self, source_event_id: &mut EventId, reason: &'static str, cx: &mut Context<Self>) -> impl Future<Output = EventId> {
        async move {
            let current_event_id = self.get_next_event_id();
            self.log_emitter.send(DiscProcessLog {
                time: cx.time().to_chrono_date_time(0).unwrap().to_string(),
                event_id: current_event_id.clone(),
                source_event_id: source_event_id.clone(),
                element_name: self.element_name.clone(),
                element_type: self.element_type.clone(),
                details: DefaultDiscProcessLogType::ProcessFailure { reason }
            }).await;
            current_event_id
        }
    }
    fn log_type_process_stopped(&mut self, source_event_id: &mut EventId, reason: &'static str, cx: &mut Context<Self>) -> impl Future<Output = EventId> {
        async move {
            let current_event_id = self.get_next_event_id();
            self.log_emitter.send(DiscProcessLog {
                time: cx.time().to_chrono_date_time(0).unwrap().to_string(),
                event_id: current_event_id.clone(),
                source_event_id: source_event_id.clone(),
                element_name: self.element_name.clone(),
                element_type: self.element_type.clone(),
                details: DefaultDiscProcessLogType::ProcessStopped { reason }
            }).await;
            current_event_id
        }
    }
    fn log_type_state_change(&mut self, source_event_id: &mut EventId, new_state: DiscStockState, cx: &mut Context<Self>) -> impl Future<Output = EventId> {
        async move {
            let current_event_id = self.get_next_event_id();
            self.log_emitter.send(DiscProcessLog {
                time: cx.time().to_chrono_date_time(0).unwrap().to_string(),
                event_id: current_event_id.clone(),
                source_event_id: source_event_id.clone(),
                element_name: self.element_name.clone(),
                element_type: self.element_type.clone(),
                details: DefaultDiscProcessLogType::StateChange { new_state }
            }).await;
            current_event_id
        }
    }
}

struct WorkstationProcess {
    // Identification
    pub element_name: String,
    pub element_code: String,
    pub element_type: String,

    // Ports
    pub req_worker: Requestor<(), DiscStockState>,
    pub seize_worker: Requestor<(), Worker>,
    pub release_worker: Output<Worker>,

    pub req_job: Requestor<(), DiscStockState>,
    pub seize_job: Requestor<(), Job>,
    pub release_job: Output<Job>,

    pub log_emitter: Output<DiscProcessLog<DefaultDiscProcessLogType<Worker>>>,

    // Configuration
    pub process_time_factor_distr: Distribution,

    // Runtime state
    pub process_state: Option<(Duration, Worker, Job)>,

    // Internals
    pub time_to_next_process_event: Option<Duration>,
    pub scheduled_event: Option<(MonotonicTime, ActionKey)>,
    pub next_event_index: u64,
    pub previous_check_time: MonotonicTime,
}

impl Connect<WorkerShiftManager, DefaultDiscStock<Worker, DiscStockState, DiscStockLog<Worker>>> for Connection {
    fn connect(
        &mut self,
        a: (&mut WorkerShiftManager, &Address<WorkerShiftManager>),
        b: (&mut DefaultDiscStock<Worker, DiscStockState, DiscStockLog<Worker>>, &Address<DefaultDiscStock<Worker, DiscStockState, DiscStockLog<Worker>>>),
    ) -> Result<(), String> {
        a.0.add_worker.map_connect(|(worker, event_id)| (Some(*worker), *event_id),DefaultDiscStock::add_one, b.1.clone());
        a.0.remove_worker.connect(DefaultDiscStock::remove_one, b.1.clone());
        Ok(())
    }
}

fn create_bench() {
    let mut df = DistributionFactory::new(7_777_777);

    // Component declarations

    // let mut source: DefaultDiscSource<String, DiscProcessLog<DefaultDiscProcessLogType<Worker>>> = DefaultDiscSource::new()
    //     .with_name("Source")
    //     .with_code("SRC")
    //     .with_source_time_distr(df.create(DistributionConfig::Constant(0.5)).unwrap());
    // let src_mbox = Mailbox::new();
    // let src_addr = src_mbox.address();
    // source.source_item_generator = Some(Box::new(SimpleStringGenerator::new("ITEM_{}".into())));

    // let mut queue_1: DefaultDiscStock<String, DiscStockState, DiscStockLog<String>> = DefaultDiscStock::new()
    //     .with_name("Queue 1")
    //     .with_code("Q1")
    //     .with_max_capacity(10);
    // queue_1.resources().add_multi(["A1", "A2", "A3"].iter().map(|s| s.to_string()).collect());

    // let q1_mbox = Mailbox::new();
    // let q1_addr = q1_mbox.address();

    // let mut process: DefaultDiscProcess<String, DiscProcessLog<DefaultDiscProcessLogType<Worker>>> = DefaultDiscProcess::new()
    //     .with_name("Process")
    //     .with_code("P")
    //     .with_process_time_distr(df.create(DistributionConfig::Constant(0.1)).unwrap());
    // let p_mbox = Mailbox::new();
    // let p_addr = p_mbox.address();

    // let mut queue_2: DefaultDiscStock<String, DiscStockState, DiscStockLog<String>> = DefaultDiscStock::new()
    //     .with_name("Queue 2")
    //     .with_code("Q2")
    //     .with_max_capacity(10);
    // let q2_mbox = Mailbox::new();
    // let q2_addr = q2_mbox.address();

    // let mut sink : DefaultDiscSink<String, DiscProcessLog<DefaultDiscProcessLogType<Worker>>> = DefaultDiscSink::new()
    //     .with_name("Sink")
    //     .with_code("SNK")
    //     .with_sink_time_distr(df.create(DistributionConfig::Constant(0.5)).unwrap());
    // let snk_mbox = Mailbox::new();
    // let snk_addr = snk_mbox.address();

    let mut worker_shift_manager = WorkerShiftManager {
        element_name: "WorkerShiftManager".to_string(),
        element_code: "WSM".to_string(),
        element_type: "WorkerShiftManager".to_string(),
        add_worker: Output::new(),
        remove_worker: Requestor::new(),
        log_emitter: Output::new(),
        weekly_work_schedule: vec![
            (Worker::new("Worker1"), Duration::from_secs(8 * 3600), Duration::from_secs(17 * 3600)),
            (Worker::new("Worker2"), Duration::from_secs(8 * 3600), Duration::from_secs(17 * 3600)),
            (Worker::new("Worker3"), Duration::from_secs(8 * 3600), Duration::from_secs(17 * 3600)),
            (Worker::new("Worker4"), Duration::from_secs(8 * 3600), Duration::from_secs(17 * 3600)),
            (Worker::new("Worker5"), Duration::from_secs(8 * 3600), Duration::from_secs(17 * 3600)),
        ],
        current_workers: vec![],
        time_to_next_process_event: None,
        scheduled_event: None,
        next_event_index: 0,
        previous_check_time: MonotonicTime::MIN,
    };
    let wsm_mbox = Mailbox::new();
    let wsm_addr = wsm_mbox.address();

    let mut available_workers: DefaultDiscStock<Worker, DiscStockState, DiscStockLog<Worker>> = DefaultDiscStock::new()
        .with_name("AvailableWorkers")
        .with_code("AW")
        .with_max_capacity(100);

    // Connections

    let mut c = Connection {};

}