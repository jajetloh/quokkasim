/*
 * Simulation of staff operations in a workshop, including parallel workstations
 * and staffing constraints.
 */

use quokkasim::prelude::*;
use serde::Serialize;
use std::{time::{Duration, SystemTime}};

#[derive(Clone, Serialize, Debug)]
struct Worker(String); // Simple wrapper type for worker resource

#[derive(Clone, Serialize, Debug)]
struct Job(String); // Simple wrapper type for job resource

struct WorkerShiftManager {
    // Identification
    pub element_name: String,
    pub element_code: String,
    pub element_type: String,

    // Ports
    pub add_worker: Output<Worker>,
    pub remove_worker: Requestor<(), Worker>,
    pub log_emitter: Output<DiscProcessLog<DefaultDiscProcessLogType<String>, String>>,
    pub req_environment: Requestor<(), BasicEnvironmentState>,

    // Configuration
    pub weekly_work_schedule: Vec<(u32, Duration, Duration)>,
    pub delay_modes: DelayModes,

    // Runtime state
    pub current_workers: Vec<(Duration, Worker)>,
    pub env_state: BasicEnvironmentState,

    // Internals
    pub time_to_next_process_event: Option<Duration>,
    pub time_to_next_delay_event: Option<Duration>,
    pub scheduled_event: Option<(MonotonicTime, ActionKey)>,
    pub next_event_index: u64,
    pub previous_check_time: MonotonicTime,
}

impl DiscProcessCore<Worker, DiscProcessLog<DefaultDiscProcessLogType<Worker>, Worker>, DefaultDiscProcessLogType<Worker>> for WorkerShiftManager {
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
    fn log_emitter(&mut self) -> &mut Output<DiscProcessLog<DefaultDiscProcessLogType<Worker>, Worker>> { &mut self.log_emitter }
    fn scheduled_event(&mut self) -> &mut Option<(MonotonicTime, ActionKey)> {
        &mut self.scheduled_event
    }
    fn time_to_next_process_event(&mut self) -> &mut Option<Duration> {
        &mut self.time_to_next_process_event
    }
    fn previous_check_time(&mut self) -> &mut MonotonicTime {
        &mut self.previous_check_time
    }
    fn delay_modes(&mut self) -> &mut DelayModes {
        &mut self.delay_modes
    }
    fn process_state(&mut self) -> &mut Option<Vec<(Duration, Worker)>> {
        &mut self.current_workers
    }
    fn env_state(&mut self) -> &mut BasicEnvironmentState {
        &mut self.env_state
    }
    fn req_environment(&mut self) -> &mut Requestor<(), BasicEnvironmentState> {
        &mut self.req_environment
    }
    fn time_to_next_delay_event(&mut self) -> &mut Option<Duration> {
        &mut self.time_to_next_delay_event
    }
    fn log_type_process_continue(&self, reason: &'static str) -> DefaultDiscProcessLogType<Worker> {
        DefaultDiscProcessLogType::ProcessContinue(reason)
    }
    fn log_type_withdraw_request(&self, reason: &'static str) -> DefaultDiscProcessLogType<Worker> {
        DefaultDiscProcessLogType::WithdrawRequest(reason)
    }
    fn log_type_process_success(&self, result: Worker) -> DefaultDiscProcessLogType<Worker> {
        DefaultDiscProcessLogType::Success(result)
    }
    fn log_type_process_start(&self, item: Worker) -> DefaultDiscProcessLogType<Worker> {
        DefaultDiscProcessLogType::Start(item)
    }
    fn log_type_process_failure(&self, reason: &'static str) -> DefaultDiscProcessLogType<Worker> {
        DefaultDiscProcessLogType::ProcessFailure(reason)
    }
    fn log_type_process_stopped(&self, reason: &'static str) -> DefaultDiscProcessLogType<Worker> {
        DefaultDiscProcessLogType::Stopped(reason)
    }
    fn log_type_delay_start(&self, reason: &'static str) -> DefaultDiscProcessLogType<Worker> {
        DefaultDiscProcessLogType::DelayStart(reason)
    }
    fn log_type_delay_end(&self, reason: &'static str) -> DefaultDiscProcessLogType<Worker> {
        DefaultDiscProcessLogType::DelayEnd(reason)
    }
    fn log_type_state_change(&self, new_state: DiscStockState) -> DefaultDiscProcessLogType<Worker> {
        DefaultDiscProcessLogType::StateChange(new_state)
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

    pub log_emitter: Output<DiscProcessLog<DefaultDiscProcessLogType<String>, String>>,

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

fn create_bench() {
    let mut df = DistributionFactory::new(7_777_777);

    // Component declarations

    let mut source: DefaultDiscSource<String, DiscProcessLog<DefaultDiscProcessLogType<String>, String>> = DefaultDiscSource::new()
        .with_name("Source")
        .with_code("SRC")
        .with_source_time_distr(df.create(DistributionConfig::Constant(0.5)).unwrap());
    let src_mbox = Mailbox::new();
    let src_addr = src_mbox.address();
    source.source_item_generator = Some(Box::new(SimpleStringGenerator::new("ITEM_{}".into())));

    let mut queue_1: DefaultDiscStock<String, DiscStockState, DiscStockLog<String>> = DefaultDiscStock::new()
        .with_name("Queue 1")
        .with_code("Q1")
        .with_max_capacity(10);
    queue_1.resources().add_multi(["A1", "A2", "A3"].iter().map(|s| s.to_string()).collect());

    let q1_mbox = Mailbox::new();
    let q1_addr = q1_mbox.address();

    let mut process: DefaultDiscProcess<String, DiscProcessLog<DefaultDiscProcessLogType<String>, String>> = DefaultDiscProcess::new()
        .with_name("Process")
        .with_code("P")
        .with_process_time_distr(df.create(DistributionConfig::Constant(0.1)).unwrap());
    let p_mbox = Mailbox::new();
    let p_addr = p_mbox.address();

    let mut queue_2: DefaultDiscStock<String, DiscStockState, DiscStockLog<String>> = DefaultDiscStock::new()
        .with_name("Queue 2")
        .with_code("Q2")
        .with_max_capacity(10);
    let q2_mbox = Mailbox::new();
    let q2_addr = q2_mbox.address();

    let mut sink : DefaultDiscSink<String, DiscProcessLog<DefaultDiscProcessLogType<String>, String>> = DefaultDiscSink::new()
        .with_name("Sink")
        .with_code("SNK")
        .with_sink_time_distr(df.create(DistributionConfig::Constant(0.5)).unwrap());
    let snk_mbox = Mailbox::new();
    let snk_addr = snk_mbox.address();

    // Connections

    let mut c = Connection {};

}