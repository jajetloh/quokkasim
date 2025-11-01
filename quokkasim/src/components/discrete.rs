use std::time::Duration;
use std::fmt::Debug;

use nexosim::ports::Output;
use serde::Serialize;
use crate::prelude::*;

#[derive(Serialize, Clone, Debug)]
pub enum DiscStockLogType<T> {
    AddOne { balance: usize, added: Option<T> },
    AddMulti { balance: usize, added: Vec<T> },
    RemoveOne { balance: usize, removed: Option<T> },
    RemoveMulti { balance: usize, removed: Vec<T> },
    StateChange { new_state: DiscStockState },
}

#[derive(WithMethods)]
pub struct DefaultDiscStock<ItemType, S: StockState, RecordLogType: Clone + Send + 'static> {

    // Identification
    pub element_name: String,
    pub element_code: String,
    pub element_type: String,

    // Ports
    pub log_emitter: Output<RecordLogType>,
    pub state_emitter: Output<EventId>,

    // Configuration
    pub low_capacity: usize,
    pub max_capacity: usize,

    // Runtime State
    pub resources: VecDequeStock<ItemType>,

    // Internals
    prev_state: Option<S>,
    next_event_index: usize,
}

impl<
    ItemType: Clone + Debug + Serialize + Send + 'static,
    LogRecordType: Clone + Send + 'static,
> Model for DefaultDiscStock<
    ItemType,
    DiscStockState,
    LogRecordType,
> {}

impl<T: 'static, S: StockState + Send + 'static, RecordLogType: Clone + Send + 'static> Default for DefaultDiscStock<
    T,
    S, 
    RecordLogType,
> {
    fn default() -> Self {
        DefaultDiscStock {
            element_name: "DefaultDiscStock".into(),
            element_code: "".into(),
            element_type: "DefaultDiscStock".into(),

            log_emitter: Output::default(),
            state_emitter: Output::default(),

            low_capacity: 0,
            max_capacity: usize::MAX,

            resources: VecDequeStock::new(VecDequeAccess::FIFO),

            prev_state: None,
            next_event_index: 0,
        }
    }
}

impl<
    ItemType: Clone + Debug + Serialize + Send + 'static,
> DiscStock<
    ItemType,
    DiscStockState,
> for DefaultDiscStock<ItemType, DiscStockState, DiscStockLog<ItemType>> 
{
    fn get_state(&mut self) -> DiscStockState {
        let occupied = self.resources.total();
        let empty = self.max_capacity.saturating_sub(occupied);
        if occupied == 0 {
            DiscStockState::Empty { occupied, empty }
        } else if empty == 0 {
            DiscStockState::Full { occupied, empty }
        } else {
            DiscStockState::Normal { occupied, empty }
        }
    }

    fn get_next_event_id(&mut self) -> EventId {
        let event_id = EventId(format!(
            "{}_{:06}",
            self.element_code, self.next_event_index
        ));
        self.next_event_index += 1;
        event_id
    }

    fn previous_state(&mut self) -> &mut Option<DiscStockState> {
        &mut self.prev_state
    }
    fn resources(&mut self) -> &mut VecDequeStock<ItemType> {
        &mut self.resources
    }
    fn state_emitter(&mut self) -> &mut Output<EventId> {
        &mut self.state_emitter
    }
    fn log_type_add_one(&mut self, source_event_id: &mut EventId, balance: usize, resource: Option<ItemType>, cx: &mut Context<Self>) -> impl Future<Output = EventId> {
        async move {
            let current_event_id = self.get_next_event_id();
            self.log_emitter.send(DiscStockLog {
                time: cx.time().to_chrono_date_time(0).unwrap().to_string(),
                event_id: current_event_id.clone(),
                source_event_id: source_event_id.clone(),
                element_name: self.element_name.clone(),
                element_type: self.element_type.clone(),
                details: DiscStockLogType::AddOne { balance, added: resource.clone() },
            }).await;
            current_event_id
        }
    }
    fn log_type_add_multi(&mut self, source_event_id: &mut EventId, balance: usize, resource: Vec<ItemType>, cx: &mut Context<Self>) -> impl Future<Output = EventId> {
        async move {
            let current_event_id = self.get_next_event_id();
            self.log_emitter.send(DiscStockLog {
                time: cx.time().to_chrono_date_time(0).unwrap().to_string(),
                event_id: current_event_id.clone(),
                source_event_id: source_event_id.clone(),
                element_name: self.element_name.clone(),
                element_type: self.element_type.clone(),
                details: DiscStockLogType::AddMulti { balance, added: resource.clone() },
            }).await;
            current_event_id
        }
    }
    fn log_type_remove_one(&mut self, source_event_id: &mut EventId, balance: usize, resource: Option<ItemType>, cx: &mut Context<Self>) -> impl Future<Output = EventId> {
        async move {
            let current_event_id = self.get_next_event_id();
            self.log_emitter.send(DiscStockLog {
                time: cx.time().to_chrono_date_time(0).unwrap().to_string(),
                event_id: current_event_id.clone(),
                source_event_id: source_event_id.clone(),
                element_name: self.element_name.clone(),
                element_type: self.element_type.clone(),
                details: DiscStockLogType::RemoveOne { balance, removed: resource.clone() },
            }).await;
            current_event_id
        }
    }
    fn log_type_remove_multi(&mut self, source_event_id: &mut EventId, balance: usize, resource: Vec<ItemType>, cx: &mut Context<Self>) -> impl Future<Output = EventId> {
        async move {
            let current_event_id = self.get_next_event_id();
            self.log_emitter.send(DiscStockLog {
                time: cx.time().to_chrono_date_time(0).unwrap().to_string(),
                event_id: current_event_id.clone(),
                source_event_id: source_event_id.clone(),
                element_name: self.element_name.clone(),
                element_type: self.element_type.clone(),
                details: DiscStockLogType::RemoveMulti { balance, removed: resource.clone() },
            }).await;
            current_event_id
        }
    }
    fn log_type_state_change(&mut self, source_event_id: &mut EventId, new_state: DiscStockState, cx: &mut Context<Self>) -> impl Future<Output = EventId> {
        async move {
            let current_event_id = self.get_next_event_id();
            self.log_emitter.send(DiscStockLog {
                time: cx.time().to_chrono_date_time(0).unwrap().to_string(),
                event_id: current_event_id.clone(),
                source_event_id: source_event_id.clone(),
                element_name: self.element_name.clone(),
                element_type: self.element_type.clone(),
                details: DiscStockLogType::StateChange { new_state: new_state.clone() },
            }).await;
            current_event_id
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DiscStockLog<ItemType> {
    pub time: String,
    pub event_id: EventId,
    pub source_event_id: EventId,
    pub element_name: String,
    pub element_type: String,
    pub details: DiscStockLogType<ItemType>,
}

#[derive(WithMethods)]
pub struct DefaultDiscProcess<
    ItemType,
    ProcessLog,
> where
    ItemType: Clone + Debug + Serialize + Send + 'static,
    ProcessLog: Clone + Debug + Serialize + Send + 'static,
{
    // Identification
    pub element_name: String,
    pub element_code: String,
    pub element_type: String,

    // Ports
    pub req_upstream: Requestor<(), DiscStockState>,
    pub req_downstream: Requestor<(), DiscStockState>,
    pub withdraw_upstream: Requestor<(usize, EventId), Vec<ItemType>>,
    pub push_downstream: Output<(Vec<ItemType>, EventId)>,
    pub log_emitter: Output<ProcessLog>,

    // Configuration
    pub process_quantity_distr: Distribution,
    pub process_time_distr: Distribution,

    // Runtime state
    pub process_state: Option<(Duration, Vec<ItemType>)>,

    // Internals
    pub time_to_next_process_event: Option<Duration>,
    pub scheduled_event: Option<(MonotonicTime, ActionKey)>,
    pub next_event_index: u64,
    pub previous_check_time: MonotonicTime,
}

impl<ItemType: Clone + Debug + Serialize + Send + 'static> Model for DefaultDiscProcess<
    ItemType, 
    DiscProcessLog<ItemType>
>
where
    DiscProcessLog<ItemType>: Serialize
{
    fn init(
        mut self,
        ctx: &mut Context<Self>,
    ) -> impl Future<Output = InitializedModel<Self>> + Send {
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

impl<
        ItemType: Clone + Debug + Serialize + Send + 'static,
        ProcessLog: Clone + Debug + Serialize + Send + 'static,
    > Default for DefaultDiscProcess<ItemType, ProcessLog>
{
    fn default() -> Self {
        DefaultDiscProcess {
            element_name: "DefaultDiscProcess".into(),
            element_code: "".into(),
            element_type: "DefaultDiscProcess".into(),

            req_upstream: Requestor::default(),
            req_downstream: Requestor::default(),
            withdraw_upstream: Requestor::default(),
            push_downstream: Output::default(),
            log_emitter: Output::default(),

            process_quantity_distr: Distribution::default(),
            process_time_distr: Distribution::default(),

            process_state: None,

            time_to_next_process_event: None,
            scheduled_event: None,
            next_event_index: 0,
            previous_check_time: MonotonicTime::EPOCH,
        }
    }
}

impl<ItemType> DiscProcessCore<
    ItemType,
    DiscProcessLog<ItemType>,
> for DefaultDiscProcess<
    ItemType,
    DiscProcessLog<ItemType>,
>
where
    ItemType: Clone + Debug + Serialize + Send + 'static,
{
    fn update_state(
            &mut self,
            source_event_id: EventId,
            cx: &mut Context<Self>,
        ) -> impl Future<Output = ()> {
        async move {
            self.update_state_since_last_update(&mut source_event_id.clone(), cx)
                .await;
            self.update_state_decision_logic(&mut source_event_id.clone(), cx)
                .await;
            self.update_state_for_next_event(&mut source_event_id.clone(), cx)
                .await;
        }
    }

    fn element_name(&self) -> &str {
        &self.element_name
    }

    fn element_code(&self) -> &str {
        &self.element_code
    }

    fn element_type(&self) -> &str {
        &self.element_type
    }

    fn get_next_event_id(&mut self) -> EventId {
        let id = EventId(format!(
            "{}_{:06}",
            self.element_code, self.next_event_index
        ));
        self.next_event_index += 1;
        id
    }

    fn scheduled_event(
        &mut self,
    ) -> &mut Option<(MonotonicTime, ActionKey)> {
        &mut self.scheduled_event
    }

    fn previous_check_time(&mut self) -> &mut MonotonicTime {
        &mut self.previous_check_time
    }

    fn time_to_next_process_event(
        &mut self,
    ) -> &mut Option<Duration> {
        &mut self.time_to_next_process_event
    }
}

impl<
    ItemType,
> DiscProcessUpdateDecisionLogic<
    ItemType,
    DiscProcessLog<ItemType>,
> for DefaultDiscProcess<
        ItemType,
        DiscProcessLog<ItemType>,
    >
where
    ItemType: Clone + Debug + Serialize + Send + 'static,
    Self: DiscProcessCore<
        ItemType,
        DiscProcessLog<ItemType>,
    >
{
    fn update_state_decision_logic(
        &mut self,
        source_event_id: &mut EventId,
        cx: &mut Context<Self>,
    ) -> impl Future<Output = ()> {
        async move {
            match self.process_state {
                None => {
                    let upstream_state = self.req_upstream.send(()).await.next();
                    let downstream_state =
                        self.req_downstream.send(()).await.next();

                    match (&upstream_state, &downstream_state) {
                        (
                            Some(DiscStockState::Normal { .. })
                            | Some(DiscStockState::Full { .. }),
                            Some(DiscStockState::Empty { .. })
                            | Some(DiscStockState::Normal { .. }),
                        ) => {
                            let mut requested =
                                self.process_quantity_distr.sample().round();
                            if !requested.is_finite() {
                                requested = 1.0;
                            }
                            let requested = requested.clamp(1.0, u32::MAX as f64) as usize;

                            *source_event_id = self.log_type_withdraw_request(source_event_id, requested, cx).await;

                            let pulled = self
                                .withdraw_upstream
                                .send((requested, source_event_id.clone()))
                                .await
                                .next();

                            match pulled {
                                Some(batch) => {
                                    if batch.is_empty() {
                                        *source_event_id = self.log_type_process_failure(source_event_id, "Upstream returned no items", cx).await;
                                        self.time_to_next_process_event = None;
                                        return;
                                    }

                                    let mut process_secs =
                                        self.process_time_distr.sample();
                                    if !process_secs.is_finite() {
                                        process_secs = 0.0;
                                    }
                                    process_secs = process_secs.max(0.0);
                                    let mut process_duration =
                                        Duration::from_secs_f64(process_secs);
                                    if process_duration.is_zero() {
                                        process_duration = Duration::from_nanos(1);
                                    }

                                    let quantity = batch.len();
                                    let log_payload = batch.clone();

                                    self.process_state =
                                        Some((process_duration, batch));
                                    *source_event_id = self.log_type_process_start(source_event_id, quantity, log_payload, cx).await;
                                    self.time_to_next_process_event =
                                        Some(process_duration);
                                }
                                None => {
                                    *source_event_id = self.log_type_process_failure(source_event_id, "Upstream requestor closed", cx).await;
                                    self.time_to_next_process_event = None;
                                }
                            }
                        }
                        (Some(DiscStockState::Empty { .. }), _) => {
                            *source_event_id = self.log_type_process_failure(source_event_id, "Upstream is empty", cx).await;
                            self.time_to_next_process_event = None;
                        }
                        (None, _) => {
                            *source_event_id = self.log_type_process_failure(source_event_id, "Upstream is not connected", cx).await;
                            self.time_to_next_process_event = None;
                        }
                        (_, None) => {
                            *source_event_id = self.log_type_process_failure(source_event_id, "Downstream is not connected", cx).await;
                            self.time_to_next_process_event = None;
                        }
                        (_, Some(DiscStockState::Full { .. })) => {
                            *source_event_id = self.log_type_process_failure(source_event_id, "Downstream is full", cx).await;
                            self.time_to_next_process_event = None;
                        }
                    }
                }
                Some((time_left, _)) => {
                    self.time_to_next_process_event = Some(time_left);
                }
            }
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
                details: DefaultDiscProcessLogType::WithdrawRequest { quantity },
            }).await;
            current_event_id
        }
    }
    
    fn log_type_process_start(&mut self, source_event_id: &mut EventId, quantity: usize, resources: Vec<ItemType>, cx: &mut Context<Self>) -> impl Future<Output = EventId> {
        async move {
            let current_event_id = self.get_next_event_id();
            self.log_emitter.send(DiscProcessLog {
                time: cx.time().to_chrono_date_time(0).unwrap().to_string(),
                event_id: current_event_id.clone(),
                source_event_id: source_event_id.clone(),
                element_name: self.element_name.clone(),
                element_type: self.element_type.clone(),
                details: DefaultDiscProcessLogType::ProcessStart { quantity, resources },
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
                details: DefaultDiscProcessLogType::ProcessFailure { reason },
            }).await;
            current_event_id
        }
    }
}

impl<
    ItemType,
> DiscProcessUpdateSinceLast<
    ItemType,
    DiscProcessLog<ItemType>,
    > for DefaultDiscProcess<
        ItemType,
        DiscProcessLog<ItemType>,
    >
where
    ItemType: Clone + Debug + Serialize + Send + 'static,
    Self: DiscProcessCore<
        ItemType,
        DiscProcessLog<ItemType>,
    >
{
    fn update_process_state_since_prev_event(
        &mut self,
        source_event_id: &mut EventId,
        cx: &mut Context<Self>,
        duration_since_prev: Duration
    ) -> impl Future<Output = ()> {
        async move {
            if let Some((mut time_left, resources)) = self.process_state.take() {
                time_left = time_left.saturating_sub(duration_since_prev);
                if time_left.is_zero() {
                    let log_payload = resources.clone();
                    *source_event_id = self.log_type_process_success(source_event_id, 1, log_payload, cx).await;
                    self.push_downstream
                        .send((resources, source_event_id.clone()))
                        .await;
                    self.time_to_next_process_event = None;
                } else {
                    self.process_state = Some((time_left, resources));
                    self.time_to_next_process_event = Some(time_left);
                }
            }
        }
    }

    fn log_type_process_success(&mut self, source_event_id: &mut EventId, quantity: usize, resources: Vec<ItemType>, cx: &mut Context<Self>) -> impl Future<Output = EventId> {
        async move {
            let current_event_id = self.get_next_event_id();
            self.log_emitter.send(DiscProcessLog {
                time: cx.time().to_chrono_date_time(0).unwrap().to_string(),
                event_id: current_event_id.clone(),
                source_event_id: source_event_id.clone(),
                element_name: self.element_name.clone(),
                element_type: self.element_type.clone(),
                details: DefaultDiscProcessLogType::ProcessSuccess { quantity, resources },
            }).await;
            current_event_id
        }
    }
}

impl<
    ItemType,
> DiscProcessUpdateForNextEvent<
    ItemType,
    DiscProcessLog<ItemType>,
> for DefaultDiscProcess<
    ItemType,
    DiscProcessLog<ItemType>,
>
where
    ItemType: Clone + Debug + Serialize + Send + 'static,
Self: DiscProcessCore<
    ItemType,
    DiscProcessLog<ItemType>,
> {}

pub struct SimpleStringGenerator {
    pub format: String,
    pub counter: u64,
}

impl SimpleStringGenerator {
    pub fn new(format: String) -> Self {
        SimpleStringGenerator { format, counter: 0 }
    }
}

impl Generator<String> for SimpleStringGenerator {
    fn next(&mut self) -> String {
        self.counter += 1;
        self.format.replace("{}", &self.counter.to_string())
    }
}

#[derive(WithMethods)]
pub struct DefaultDiscSource<
    ItemType,
    ProcessLog,
> where
    ItemType: Clone + Debug + Serialize + Send + 'static,
    ProcessLog: Clone + Debug + Serialize + Send + 'static,
{
    // Identification
    pub element_name: String,
    pub element_code: String,
    pub element_type: String,

    // Ports
    pub req_downstream: Requestor<(), DiscStockState>,
    pub push_downstream: Output<(Vec<ItemType>, EventId)>,
    pub log_emitter: Output<ProcessLog>,

    // Configuration
    pub source_item_generator: Option<Box<dyn Generator<ItemType> + Send>>,
    pub source_quantity_distr: Distribution,
    pub source_time_distr: Distribution,

    // Runtime state
    pub process_state: Option<(Duration, Vec<ItemType>)>,

    // Internals
    pub time_to_next_process_event: Option<Duration>,
    pub scheduled_event: Option<(MonotonicTime, ActionKey)>,
    pub next_event_index: usize,
    pub previous_check_time: MonotonicTime,
}

impl<
        ItemType: Clone + Debug + Serialize + Send + 'static,
        ProcessLog: Clone + Debug + Serialize + Send + 'static,
    > Default for DefaultDiscSource<ItemType, ProcessLog>
{
    fn default() -> Self {
        DefaultDiscSource {
            element_name: "DefaultDiscSource".into(),
            element_code: "".into(),
            element_type: "DefaultDiscSource".into(),

            req_downstream: Requestor::default(),
            push_downstream: Output::default(),
            log_emitter: Output::default(),

            source_item_generator: None,
            source_quantity_distr: Distribution::default(),
            source_time_distr: Distribution::default(),

            process_state: None,

            time_to_next_process_event: None,
            scheduled_event: None,
            next_event_index: 0,
            previous_check_time: MonotonicTime::EPOCH,
        }
    }
}

impl<
    ItemType: Clone + Debug + Serialize + Send + 'static,
> DiscProcessUpdateSinceLast<
    ItemType,
    DiscProcessLog<ItemType>,
> for DefaultDiscSource<
    ItemType,
    DiscProcessLog<ItemType>,
>
where   
    ItemType: Clone + Debug + Serialize + Send + 'static,
    Self: DiscProcessCore<
        ItemType,
        DiscProcessLog<ItemType>,
    >,
{
    fn update_process_state_since_prev_event(
        &mut self, source_event_id: &mut EventId,
        cx: &mut Context<Self>,
        duration_since_prev: Duration
    ) -> impl Future<Output = ()> {
        async move {
            if let Some((mut time_left, resources)) = self.process_state.take() {
                time_left = time_left.saturating_sub(duration_since_prev);
                if time_left.is_zero() {
                    let quantity = resources.len();
                    let log_payload = resources.clone();
                    *source_event_id = self.log_type_process_success(source_event_id, quantity, log_payload, cx).await;
                    self.push_downstream
                        .send((resources, source_event_id.clone()))
                        .await;
                    self.time_to_next_process_event = None;
                } else {
                    self.process_state = Some((time_left, resources));
                    self.time_to_next_process_event = Some(time_left);
                }
            }
        }
    }

    fn log_type_process_success(&mut self, source_event_id: &mut EventId, quantity: usize, resources: Vec<ItemType>, cx: &mut Context<Self>) -> impl Future<Output = EventId> {
        async move {
            let current_event_id = self.get_next_event_id();
            self.log_emitter.send(DiscProcessLog {
                time: cx.time().to_chrono_date_time(0).unwrap().to_string(),
                event_id: current_event_id.clone(),
                source_event_id: source_event_id.clone(),
                element_name: self.element_name.clone(),
                element_type: self.element_type.clone(),
                details: DefaultDiscProcessLogType::ProcessSuccess { quantity, resources },
            }).await;
            current_event_id
        }
    }
}

impl<
    ItemType: Clone + Debug + Serialize + Send + 'static,
> DiscProcessUpdateDecisionLogic<
    ItemType,
    DiscProcessLog<ItemType>,
> for DefaultDiscSource<
    ItemType,
    DiscProcessLog<ItemType>,
> 
where 
    ItemType: Clone + Debug + Serialize + Send + 'static,
    Self: DiscProcessCore<
        ItemType,
        DiscProcessLog<ItemType>,
>
{
    fn update_state_decision_logic(
            &mut self,
            source_event_id: &mut EventId,
            cx: &mut Context<Self>,
        ) -> impl Future<Output = ()> {
        async move {
            match self.process_state {
                None => {
                    let downstream_state = self.req_downstream.send(()).await.next();

                    match &downstream_state {
                        Some(DiscStockState::Empty { .. })
                        | Some(DiscStockState::Normal { .. }) => {
                            let mut requested =
                                self.source_quantity_distr.sample().round();
                            if !requested.is_finite() {
                                requested = 1.0;
                            }
                            let mut requested = requested.clamp(1.0, usize::MAX as f64) as usize;
                            if requested == 0 {
                                requested = 1;
                            }

                            let generator = match self.source_item_generator.as_mut() {
                                Some(generator) => generator.next(),
                                None => {
                                    *source_event_id = self.log_type_process_failure(source_event_id, "Source item generator not configured", cx).await;
                                    self.time_to_next_process_event = None;
                                    return;
                                }
                            };

                            let mut batch = Vec::with_capacity(requested);
                            batch.resize_with(requested, || generator.clone());
                            if batch.is_empty() {
                                *source_event_id = self.log_type_process_failure(source_event_id, "Source produced an empty batch", cx).await;
                                self.time_to_next_process_event = None;
                                return;
                            }

                            *source_event_id = self.log_type_withdraw_request(source_event_id, batch.len(), cx).await;

                            let mut process_secs = self.source_time_distr.sample();
                            if !process_secs.is_finite() {
                                process_secs = 0.0;
                            }
                            process_secs = process_secs.max(0.0);
                            let mut process_duration =
                                Duration::from_secs_f64(process_secs);
                            if process_duration.is_zero() {
                                process_duration = Duration::from_nanos(1);
                            }

                            let quantity = batch.len();
                            let log_payload = batch.clone();

                            *source_event_id = self.log_type_process_start(source_event_id, quantity, log_payload, cx).await;
                            self.process_state =
                                Some((process_duration, batch));
                            self.time_to_next_process_event =
                                Some(process_duration);
                        }
                        None => {
                            *source_event_id = self.log_type_process_failure(source_event_id, "Downstream is not connected", cx).await;
                            self.time_to_next_process_event = None;
                        }
                        Some(DiscStockState::Full { .. }) => {
                            *source_event_id = self.log_type_process_failure(source_event_id, "Downstream is full", cx).await;
                            self.time_to_next_process_event = None;
                        }
                    }
                }
                Some((time_left, _)) => {
                    self.time_to_next_process_event = Some(time_left);
                }
            }
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
                details: DefaultDiscProcessLogType::WithdrawRequest { quantity },
            }).await;
            current_event_id
        }
    }
    
    fn log_type_process_start(&mut self, source_event_id: &mut EventId, quantity: usize, resources: Vec<ItemType>, cx: &mut Context<Self>) -> impl Future<Output = EventId> {
        async move {
            let current_event_id = self.get_next_event_id();
            self.log_emitter.send(DiscProcessLog {
                time: cx.time().to_chrono_date_time(0).unwrap().to_string(),
                event_id: current_event_id.clone(),
                source_event_id: source_event_id.clone(),
                element_name: self.element_name.clone(),
                element_type: self.element_type.clone(),
                details: DefaultDiscProcessLogType::ProcessStart { quantity, resources },
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
                details: DefaultDiscProcessLogType::ProcessFailure { reason },
            }).await;
            current_event_id
        }
    }
}

impl<
    ItemType: Clone + Debug + Serialize + Send + 'static,
> DiscProcessUpdateForNextEvent<
    ItemType,
    DiscProcessLog<ItemType>,
> for DefaultDiscSource<
    ItemType,
    DiscProcessLog<ItemType>,
> 
where 
    ItemType: Clone + Debug + Serialize + Send + 'static,
    Self: DiscProcessCore<
        ItemType,
        DiscProcessLog<ItemType>,
    >
{}

impl<ItemType: Clone + Debug + Serialize + Send + 'static> Model for DefaultDiscSource<
    ItemType, 
    DiscProcessLog<ItemType>
>
where
    DiscProcessLog<DiscProcessLog<ItemType>>: Serialize,
{
    fn init(
        mut self,
        ctx: &mut Context<Self>,
    ) -> impl Future<Output = InitializedModel<Self>> + Send {
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

impl<ItemType> DiscProcessCore<
    ItemType,
    DiscProcessLog<ItemType>,
> for DefaultDiscSource<
    ItemType,
    DiscProcessLog<ItemType>,
>
where
    ItemType: Clone + Debug + Serialize + Send + 'static,
{

    fn update_state(
            &mut self,
            mut source_event_id: EventId,
            cx: &mut Context<Self>,
        ) -> impl Future<Output = ()> + Send {
        async move {
            self.update_state_since_last_update(&mut source_event_id, cx)
                .await;
            self.update_state_decision_logic(&mut source_event_id, cx)
                .await;
            self.update_state_for_next_event(&mut source_event_id, cx)
                .await;
        }
    }

    fn element_name(&self) -> &str {
        &self.element_name
    }

    fn element_code(&self) -> &str {
        &self.element_code
    }

    fn element_type(&self) -> &str {
        &self.element_type
    }

    fn get_next_event_id(&mut self) -> EventId {
        let id = EventId(format!(
            "{}_{:06}",
            self.element_code, self.next_event_index
        ));
        self.next_event_index += 1;
        id
    }

    fn scheduled_event(
        &mut self,
    ) -> &mut Option<(MonotonicTime, ActionKey)> {
        &mut self.scheduled_event
    }

    fn previous_check_time(&mut self) -> &mut MonotonicTime {
        &mut self.previous_check_time
    }

    fn time_to_next_process_event(
        &mut self,
    ) -> &mut Option<Duration> {
        &mut self.time_to_next_process_event
    }
}

#[derive(WithMethods)]
pub struct DefaultDiscSink<
    ItemType,
    ProcessLog,
> where
    ItemType: Clone + Debug + Serialize + Send + 'static,
    ProcessLog: Clone + Debug + Serialize + Send + 'static,
{
    // Identification
    pub element_name: String,
    pub element_code: String,
    pub element_type: String,

    // Ports
    pub req_upstream: Requestor<(), DiscStockState>,
    pub withdraw_upstream: Requestor<(usize, EventId), Vec<ItemType>>,
    pub log_emitter: Output<ProcessLog>,

    // Configuration
    pub sink_quantity_distr: Distribution,
    pub sink_time_distr: Distribution,

    // Runtime state
    pub process_state: Option<(Duration, Vec<ItemType>)>,

    // Internals
    pub time_to_next_process_event: Option<Duration>,
    pub scheduled_event: Option<(MonotonicTime, ActionKey)>,
    pub next_event_index: usize,
    pub previous_check_time: MonotonicTime,
}

impl<
        ItemType: Clone + Debug + Serialize + Send + 'static,
        ProcessLog: Clone + Debug + Serialize + Send + 'static,
    > Default for DefaultDiscSink<ItemType, ProcessLog>
{
    fn default() -> Self {
        DefaultDiscSink {
            element_name: "DefaultDiscSink".into(),
            element_code: "".into(),
            element_type: "DefaultDiscSink".into(),

            req_upstream: Requestor::default(),
            withdraw_upstream: Requestor::default(),
            log_emitter: Output::default(),

            sink_quantity_distr: Distribution::default(),
            sink_time_distr: Distribution::default(),

            process_state: None,

            time_to_next_process_event: None,
            scheduled_event: None,
            next_event_index: 0,
            previous_check_time: MonotonicTime::EPOCH,
        }
    }
}

impl<ItemType> DiscProcessUpdateSinceLast<
    ItemType,
    DiscProcessLog<ItemType>,
> for DefaultDiscSink<
    ItemType, DiscProcessLog<ItemType>
> where
    ItemType: Clone + Debug + Serialize + Send + 'static,
{
    fn update_process_state_since_prev_event(
            &mut self, source_event_id: &mut EventId,
            cx: &mut Context<Self>,
            duration_since_prev: Duration
        ) -> impl Future<Output = ()> {
        async move {
            if let Some((mut time_left, mut resources)) = self.process_state.take() {
                time_left = time_left.saturating_sub(duration_since_prev);
                if time_left.is_zero() {
                    let quantity = resources.len();
                    let log_payload = resources.clone();
                    *source_event_id = self.log_type_process_success(source_event_id, quantity, log_payload.clone(), cx).await;
                    resources.clear();
                    self.time_to_next_process_event = None;
                } else {
                    self.process_state = Some((time_left, resources));
                    self.time_to_next_process_event = Some(time_left);
                }
            }
        }
    }

    fn log_type_process_success(&mut self, source_event_id: &mut EventId, quantity: usize, resources: Vec<ItemType>, cx: &mut Context<Self>) -> impl Future<Output = EventId> {
        async move {
            let current_event_id = self.get_next_event_id();
            self.log_emitter.send(DiscProcessLog {
                time: cx.time().to_chrono_date_time(0).unwrap().to_string(),
                event_id: current_event_id.clone(),
                source_event_id: source_event_id.clone(),
                element_name: self.element_name.clone(),
                element_type: self.element_type.clone(),
                details: DefaultDiscProcessLogType::ProcessSuccess { quantity, resources },
            }).await;
            current_event_id
        }
    }
}

impl<
    ItemType,
    > DiscProcessUpdateDecisionLogic<
    ItemType,
    DiscProcessLog<ItemType>,
    > for DefaultDiscSink<
    ItemType,
    DiscProcessLog<ItemType>,
> where
    ItemType: Clone + Debug + Serialize + Send + 'static,
    Self: DiscProcessCore<ItemType, DiscProcessLog<ItemType>>,
{
    fn update_state_decision_logic(
        &mut self,
        source_event_id: &mut EventId,
        cx: &mut Context<Self>,
    ) -> impl Future<Output = ()> {
        async move {  
            match self.process_state {
                None => {
                    let upstream_state = self.req_upstream.send(()).await.next();

                    match &upstream_state {
                        Some(DiscStockState::Normal { .. })
                        | Some(DiscStockState::Full { .. }) => {
                            let mut requested =
                                self.sink_quantity_distr.sample().round();
                            if !requested.is_finite() {
                                requested = 1.0;
                            }
                            let mut requested = requested.clamp(1.0, usize::MAX as f64) as usize;
                            if requested == 0 {
                                requested = 1;
                            }

                            *source_event_id = self.log_type_withdraw_request(source_event_id, requested, cx).await;

                            let pulled = self
                                .withdraw_upstream
                                .send((requested, source_event_id.clone()))
                                .await
                                .next();

                            match pulled {
                                Some(batch) => {
                                    if batch.is_empty() {
                                        *source_event_id = self.log_type_process_failure(source_event_id, "Upstream returned an empty batch", cx).await;
                                        self.time_to_next_process_event = None;
                                        return;
                                    }

                                    let mut process_secs =
                                        self.sink_time_distr.sample();
                                    if !process_secs.is_finite() {
                                        process_secs = 0.0;
                                    }
                                    process_secs = process_secs.max(0.0);
                                    let mut process_duration =
                                        Duration::from_secs_f64(process_secs);
                                    if process_duration.is_zero() {
                                        process_duration = Duration::from_nanos(1);
                                    }

                                    let quantity = batch.len();
                                    let log_payload = batch.clone();

                                    *source_event_id = self.log_type_process_start(source_event_id, quantity, log_payload, cx).await;
                                    self.process_state =
                                        Some((process_duration, batch));
                                    self.time_to_next_process_event =
                                        Some(process_duration);
                                }
                                None => {
                                    *source_event_id = self.log_type_process_failure(source_event_id, "Upstream requestor closed", cx).await;
                                    self.time_to_next_process_event = None;
                                }
                            }
                        }
                        Some(DiscStockState::Empty { .. }) => {
                            *source_event_id = self.log_type_process_failure(source_event_id, "Upstream is empty", cx).await;
                            self.time_to_next_process_event = None;
                        }
                        None => {
                            *source_event_id = self.log_type_process_failure(source_event_id, "Upstream is not connected", cx).await;
                            self.time_to_next_process_event = None;
                        }
                    }
                }
                Some((time_left, _)) => {
                    self.time_to_next_process_event = Some(time_left);
                }
            }  
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
                details: DefaultDiscProcessLogType::WithdrawRequest { quantity },
            }).await;
            current_event_id
        }
    }
    
    fn log_type_process_start(&mut self, source_event_id: &mut EventId, quantity: usize, resources: Vec<ItemType>, cx: &mut Context<Self>) -> impl Future<Output = EventId> {
        async move {
            let current_event_id = self.get_next_event_id();
            self.log_emitter.send(DiscProcessLog {
                time: cx.time().to_chrono_date_time(0).unwrap().to_string(),
                event_id: current_event_id.clone(),
                source_event_id: source_event_id.clone(),
                element_name: self.element_name.clone(),
                element_type: self.element_type.clone(),
                details: DefaultDiscProcessLogType::ProcessStart { quantity, resources },
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
                details: DefaultDiscProcessLogType::ProcessFailure { reason },
            }).await;
            current_event_id
        }
    }
}

impl<
    ItemType,
    ProcessLog,
> DiscProcessUpdateForNextEvent<
    ItemType,
    ProcessLog,
> for DefaultDiscSink<
    ItemType,
    ProcessLog,
> where
    ItemType: Clone + Debug + Serialize + Send + 'static,
    ProcessLog: Clone + Debug + Serialize + Send + 'static,
    Self: DiscProcessCore<ItemType, ProcessLog>,
{}

impl<ItemType> DefaultDiscSink<ItemType, DiscProcessLog<ItemType>>
where
    ItemType: Clone + Debug + Serialize + Send + 'static,
{
    pub fn update_state(
        &mut self,
        mut source_event_id: EventId,
        cx: &mut Context<Self>,
    ) -> impl Future<Output = ()> {
        async move {
            self.update_state_since_last_update(&mut source_event_id, cx)
                .await;
            self.update_state_decision_logic(&mut source_event_id, cx)
                .await;
            self.update_state_for_next_event(&mut source_event_id, cx)
                .await;
        }
    }
}

impl<ItemType: Clone + Debug + Serialize + Send + 'static> Model for DefaultDiscSink<
    ItemType, 
    DiscProcessLog<ItemType>
>
{
    fn init(
        mut self,
        ctx: &mut Context<Self>,
    ) -> impl Future<Output = InitializedModel<Self>> + Send {
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

impl<ItemType> DiscProcessCore<
        ItemType,
        DiscProcessLog<ItemType>,
    > for DefaultDiscSink<
        ItemType,
        DiscProcessLog<ItemType>,
    >
where
    ItemType: Clone + Debug + Serialize + Send + 'static,
{
    fn update_state(
            &mut self,
            mut source_event_id: EventId,
            cx: &mut Context<Self>,
        ) -> impl Future<Output = ()> {
        async move {
            self.update_state_since_last_update(&mut source_event_id, cx)
                .await;
            self.update_state_decision_logic(&mut source_event_id, cx)
                .await;
            self.update_state_for_next_event(&mut source_event_id, cx)
                .await;
        }
    }

    fn element_name(&self) -> &str {
        &self.element_name
    }

    fn element_code(&self) -> &str {
        &self.element_code
    }

    fn element_type(&self) -> &str {
        &self.element_type
    }

    fn get_next_event_id(&mut self) -> EventId {
        let id = EventId(format!(
            "{}_{:06}",
            self.element_code, self.next_event_index
        ));
        self.next_event_index += 1;
        id
    }

    fn scheduled_event(
        &mut self,
    ) -> &mut Option<(MonotonicTime, ActionKey)> {
        &mut self.scheduled_event
    }

    fn previous_check_time(&mut self) -> &mut MonotonicTime {
        &mut self.previous_check_time
    }

    fn time_to_next_process_event(
        &mut self,
    ) -> &mut Option<Duration> {
        &mut self.time_to_next_process_event
    }
}