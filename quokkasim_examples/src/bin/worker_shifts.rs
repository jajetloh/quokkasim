/**
 * A stock representing a pool of available workers, managed by a custom process
 * that adds and removes workers according to a periodic schedule
 */

use quokkasim::prelude::*;
use serde::Serialize;
use std::time::{Duration, SystemTime};

#[derive(Clone, Serialize, Debug)]
struct Worker(String); // Simple wrapper type for worker resource
impl Worker {
    fn new(name: &str) -> Worker {
        Worker(name.to_string())
    }
}

struct WorkerShiftManager {
    // Identification
    pub element_name: String,
    pub element_code: String,
    pub element_type: String,

    // Ports
    pub add_worker: Output<(Worker, EventMetadata)>,
    pub remove_worker: Requestor<EventMetadata, Option<Worker>>,
    pub log_emitter: Output<DiscProcessLog<Worker>>,

    // Configuration
    pub weekly_work_schedule: Vec<(Duration, Duration)>,

    // Runtime state

    // Internals
    pub time_to_next_process_event: Option<Duration>,
    pub scheduled_event: Option<(MonotonicTime, ActionKey)>,
    pub previous_event: EventMetadata,
    pub previous_check_time: MonotonicTime,
}

impl Model for WorkerShiftManager {
    fn init(
        mut self,
        ctx: &mut Context<Self>,
    ) -> impl Future<Output = InitializedModel<Self>> {
        async move {
            self.previous_event = EventMetadata { source_name: self.element_name.clone(), source_code: self.element_code.clone(), index: 0 };
            for (start_time, end_time) in &self.weekly_work_schedule {
                ctx.schedule_periodic_event(*start_time, Duration::from_secs(7 * 24 * 3600), Self::add_worker, self.previous_event.clone()).unwrap();
                ctx.schedule_periodic_event(*end_time, Duration::from_secs(7 * 24 * 3600), Self::remove_worker, self.previous_event.clone()).unwrap();
            }
            self.into()
        }
    }
}

impl WorkerShiftManager {
    fn add_worker(&mut self, mut source_event: EventMetadata, cx: &mut Context<Self>) -> impl Future<Output = ()> {
        async move {
            let next_event_id = self.log_type_process_start(&mut source_event, 1, vec![Worker::new("Worker")], cx).await;
            self.add_worker.send((
                Worker::new("Worker"),
                next_event_id,
            )).await;
        }
    }

    fn remove_worker(&mut self, mut source_event: EventMetadata, cx: &mut Context<Self>) -> impl Future<Output = ()> {
        async move {
            let next_event_id = self.log_type_process_start(&mut source_event, 1, vec![Worker::new("Worker")], cx).await;
            self.remove_worker.send(next_event_id).await.next();
        }
    }

    fn log_type_process_start(&mut self, source_event: &mut EventMetadata, quantity: usize, resources: Vec<Worker>, cx: &mut Context<Self>) -> impl Future<Output = EventMetadata> {
        async move {
            let current_event = self.get_next_event_meta();
            self.log_emitter.send(DiscProcessLog {
                time: cx.time().to_chrono_date_time(0).unwrap().to_string(),
                event: current_event.clone(),
                source_event: source_event.clone(),
                element_name: self.element_name.clone(),
                element_type: self.element_type.clone(),
                details: DefaultDiscProcessLogType::ProcessStart { quantity, resources }
            }).await;
            current_event
        }
    }
}

impl DiscProcessCore<Worker, DiscProcessLog<Worker>> for WorkerShiftManager {
    fn update_state(
            &mut self,
            source_event: EventMetadata,
            cx: &mut Context<Self>,
        ) -> impl Future<Output = ()> + Send {
        async move {
        }
    }
    fn element_name(&self) -> &str { &self.element_name }
    fn element_code(&self) -> &str { &self.element_code }
    fn element_type(&self) -> &str { &self.element_type }
    fn get_next_event_meta(&mut self) -> EventMetadata {
        self.previous_event.next()
    }
    fn scheduled_event(&mut self) -> &mut Option<(MonotonicTime, ActionKey)> {
        &mut self.scheduled_event
    }
    fn time_to_next_process_event(&mut self) -> &mut Option<Duration> {
        &mut self.time_to_next_process_event
    }
    fn previous_check_time(&mut self) -> &mut MonotonicTime {
        &mut self.previous_check_time
    }
}

impl Connect<WorkerShiftManager, DefaultDiscStock<Worker, DiscStockState, DiscStockLog<Worker>>> for Connection {
    fn connect(
        &mut self,
        a: (&mut WorkerShiftManager, &Address<WorkerShiftManager>, Option<usize>),
        b: (&mut DefaultDiscStock<Worker, DiscStockState, DiscStockLog<Worker>>, &Address<DefaultDiscStock<Worker, DiscStockState, DiscStockLog<Worker>>>, Option<usize>),
    ) -> Result<(), Box<dyn std::error::Error>> {
        a.0.add_worker.map_connect(|(worker, event)| (Some(worker.clone()), event.clone()),DefaultDiscStock::add_one, b.1.clone());
        a.0.remove_worker.connect(DefaultDiscStock::remove_one, b.1.clone());
        Ok(())
    }
}

fn create_bench() {

    let mut worker_shift_manager = WorkerShiftManager {
        element_name: "WorkerShiftManager".to_string(),
        element_code: "WSM".to_string(),
        element_type: "WorkerShiftManager".to_string(),
        add_worker: Output::new(),
        remove_worker: Requestor::new(),
        log_emitter: Output::new(),
        weekly_work_schedule: vec![
            (Duration::from_secs(9 * 3600), Duration::from_secs(17 * 3600)),
        ],
        time_to_next_process_event: None,
        scheduled_event: None,
        previous_event: EventMetadata::default(),
        previous_check_time: MonotonicTime::MIN,
    };
    let wsm_mbox = Mailbox::new();
    let wsm_addr = wsm_mbox.address();

    let mut available_workers: DefaultDiscStock<Worker, DiscStockState, DiscStockLog<Worker>> = DefaultDiscStock::new()
        .with_name("AvailableWorkers")
        .with_code("AW")
        .with_max_capacity(100);
    let aw_mbox = Mailbox::new();
    let aw_addr = aw_mbox.address();

    // Connections

    let mut c = Connection {};

    c.connect(
        (&mut worker_shift_manager, &wsm_addr, None),
        (&mut available_workers, &aw_addr, None),
    ).unwrap();

    // Loggers

    let process_logger = EventQueue::<DiscProcessLog<Worker>>::new();
    worker_shift_manager.log_emitter.connect_sink(&process_logger);
    let stock_logger = EventQueue::<DiscStockLog<Worker>>::new();
    available_workers.log_emitter.connect_sink(&stock_logger);

    // Simulation initialisation

    let sim_init = SimInit::new()
        .add_model(worker_shift_manager, wsm_mbox, "WorkerShiftManager")
        .add_model(available_workers, aw_mbox, "AvailableWorkers");

    let start_time = MonotonicTime::try_from_date_time(2025, 7, 1, 0, 0, 0, 0).unwrap();
    let duration = Duration::from_secs(14 * 24 * 3600);
    let (mut sim, _) = sim_init.init(start_time).unwrap();

    let time_at_start = SystemTime::now();
    sim.step_until(start_time + duration).unwrap();
    let time_at_end = SystemTime::now();
    println!("Execution time: {:?}", time_at_end.duration_since(time_at_start));

    for log in process_logger.into_reader() {
        println!("{:?}", log);
    }

    for log in stock_logger.into_reader() {
        println!("{:?}", log);
    }

}

fn main() {
    create_bench();
}