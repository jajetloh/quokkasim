use std::time::Duration;
use std::fmt::{format, Debug};

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
    LogRecordType: Clone + Send + 'static,
> DiscStock<
    ItemType,
    DiscStockState,
    LogRecordType,
    DiscStockLogType<ItemType>,
> for DefaultDiscStock<ItemType, DiscStockState, LogRecordType> 
where 
    DiscStockLogType<ItemType>: Serialize,
    Self: ToLogRecord<DiscStockLogType<ItemType>, LogRecordType>,
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

    fn resources(&mut self) -> &mut VecDequeStock<ItemType> {
        &mut self.resources
    }

    fn log_emitter(&mut self) -> &mut Output<LogRecordType> {
        &mut self.log_emitter
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
    fn state_emitter(&mut self) -> &mut Output<EventId> {
        &mut self.state_emitter
    }
    fn log_type_add_one(&self, balance: usize, resource: Option<ItemType>) -> DiscStockLogType<ItemType> {
        DiscStockLogType::AddOne { balance, added: resource }
    }
    fn log_type_add_multi(&self, balance: usize, resource: Vec<ItemType>) -> DiscStockLogType<ItemType> {
        DiscStockLogType::AddMulti { balance, added: resource }
    }
    fn log_type_remove_one(&self, balance: usize, resource: Option<ItemType>) -> DiscStockLogType<ItemType> {
        DiscStockLogType::RemoveOne { balance, removed: resource }
    }
    fn log_type_remove_multi(&self, balance: usize, resource: Vec<ItemType>) -> DiscStockLogType<ItemType> {
        DiscStockLogType::RemoveMulti { balance, removed: resource.clone() }
    }
    fn log_type_state_change(&self, new_state: DiscStockState) -> DiscStockLogType<ItemType> {
        DiscStockLogType::StateChange { new_state }
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

impl<ItemType> DiscStockLog<ItemType> {
    fn to_log(
        time: MonotonicTime,
        event_id: EventId,
        source_event_id: EventId,
        element_name: String,
        element_type: String,
        details: DiscStockLogType<ItemType>,
    ) -> Self {
        DiscStockLog {
            time: time.to_chrono_date_time(0).unwrap().to_string(),
            event_id,
            source_event_id,
            element_name,
            element_type,
            details,
        }
    }
}

impl<ItemType> ToLogRecord<
        DiscStockLogType<ItemType>,
        DiscStockLog<ItemType>,
    > for DefaultDiscStock<
        ItemType,
        DiscStockState,
        DiscStockLog<ItemType>,
    >
where
    ItemType: Clone + Debug + Serialize + Send + 'static,
{
    fn to_record(
        &mut self,
        now: MonotonicTime,
        source_event_id: EventId,
        event_id: EventId,
        details: DiscStockLogType<ItemType>,
    ) -> DiscStockLog<ItemType> {
        DiscStockLog {
            time: now.to_chrono_date_time(0).unwrap().to_string(),
            event_id,
            source_event_id,
            element_name: self.element_name.clone(),
            element_type: self.element_type.clone(),
            details,
            // phantom: std::marker::PhantomData,
        }
    }
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
    pub req_environment: Requestor<(), BasicEnvironmentState>,
    pub withdraw_upstream: Requestor<(usize, EventId), Vec<ItemType>>,
    pub push_downstream: Output<(Vec<ItemType>, EventId)>,
    pub log_emitter: Output<ProcessLog>,

    // Configuration
    pub process_quantity_distr: Distribution,
    pub process_time_distr: Distribution,
    pub delay_modes: DelayModes,

    // Runtime state
    pub process_state: Option<(Duration, Vec<ItemType>)>,
    pub env_state: BasicEnvironmentState,

    // Internals
    pub time_to_next_process_event: Option<Duration>,
    pub time_to_next_delay_event: Option<Duration>,
    pub scheduled_event: Option<(MonotonicTime, ActionKey)>,
    pub next_event_index: u64,
    pub previous_check_time: MonotonicTime,
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
            req_environment: Requestor::default(),
            withdraw_upstream: Requestor::default(),
            push_downstream: Output::default(),
            log_emitter: Output::default(),

            process_quantity_distr: Distribution::default(),
            process_time_distr: Distribution::default(),
            delay_modes: DelayModes::default(),

            process_state: None,
            env_state: BasicEnvironmentState::Normal,

            time_to_next_process_event: None,
            time_to_next_delay_event: None,
            scheduled_event: None,
            next_event_index: 0,
            previous_check_time: MonotonicTime::EPOCH,
        }
    }
}

impl<ItemType: Clone + Debug + Serialize + Send + 'static> Model for DefaultDiscProcess<
    ItemType, 
    DiscProcessLog<DefaultDiscProcessLogType<ItemType>, ItemType>
>
where
    DiscProcessLog<DefaultDiscProcessLogType<ItemType>, ItemType>: Serialize,
    Self: ToLogRecord<
        DefaultDiscProcessLogType<ItemType>,
        DiscProcessLog<DefaultDiscProcessLogType<ItemType>, ItemType>
    > {
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

impl<ItemType> ToLogRecord<
        DefaultDiscProcessLogType<ItemType>,
        DiscProcessLog<DefaultDiscProcessLogType<ItemType>, ItemType>,
    > for DefaultDiscProcess<
        ItemType,
        DiscProcessLog<DefaultDiscProcessLogType<ItemType>, ItemType>,
    >
where
    ItemType: Clone + Debug + Serialize + Send + 'static,
{
    fn to_record(
        &mut self,
        now: MonotonicTime,
        source_event_id: EventId,
        event_id: EventId,
        details: DefaultDiscProcessLogType<ItemType>,
    ) -> DiscProcessLog<DefaultDiscProcessLogType<ItemType>, ItemType> {
        DiscProcessLog {
            time: now.to_chrono_date_time(0).unwrap().to_string(),
            event_id,
            source_event_id,
            element_name: self.element_name.clone(),
            element_type: self.element_type.clone(),
            details,
            phantom: std::marker::PhantomData,
        }
    }
}

impl<ItemType> DiscProcessCore<
        ItemType,
        DiscProcessLog<DefaultDiscProcessLogType<ItemType>, ItemType>,
        DefaultDiscProcessLogType<ItemType>,
    > for DefaultDiscProcess<
        ItemType,
        DiscProcessLog<DefaultDiscProcessLogType<ItemType>, ItemType>,
    >
where
    ItemType: Clone + Debug + Serialize + Send + 'static,
{
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

    fn log_emitter(
        &mut self,
    ) -> &mut Output<
        DiscProcessLog<DefaultDiscProcessLogType<ItemType>, ItemType>,
    > {
        &mut self.log_emitter
    }

    fn scheduled_event(
        &mut self,
    ) -> &mut Option<(MonotonicTime, ActionKey)> {
        &mut self.scheduled_event
    }

    fn previous_check_time(&mut self) -> &mut MonotonicTime {
        &mut self.previous_check_time
    }

    fn delay_modes(&mut self) -> &mut DelayModes {
        &mut self.delay_modes
    }

    fn process_state(
        &mut self,
    ) -> &mut Option<(Duration, Vec<ItemType>)> {
        &mut self.process_state
    }

    fn env_state(&mut self) -> &mut BasicEnvironmentState {
        &mut self.env_state
    }

    fn req_environment(
        &mut self,
    ) -> &mut Requestor<(), BasicEnvironmentState> {
        &mut self.req_environment
    }

    fn time_to_next_process_event(
        &mut self,
    ) -> &mut Option<Duration> {
        &mut self.time_to_next_process_event
    }

    fn time_to_next_delay_event(
        &mut self,
    ) -> &mut Option<Duration> {
        &mut self.time_to_next_delay_event
    }

    fn log_type_withdraw_request(&self, quantity: usize) -> DefaultDiscProcessLogType<ItemType> {
        DefaultDiscProcessLogType::WithdrawRequest { quantity }
    }

    fn log_type_process_start(
        &self,
        quantity: usize,
        resources: Vec<ItemType>,
    ) -> DefaultDiscProcessLogType<ItemType> {
        DefaultDiscProcessLogType::ProcessStart { quantity, resources }
    }

    fn log_type_process_success(
        &self,
        quantity: usize,
        resources: Vec<ItemType>,
    ) -> DefaultDiscProcessLogType<ItemType> {
        DefaultDiscProcessLogType::ProcessSuccess { quantity, resources }
    }

    fn log_type_process_failure(
        &self,
        reason: &'static str,
    ) -> DefaultDiscProcessLogType<ItemType> {
        DefaultDiscProcessLogType::ProcessFailure { reason }
    }

    fn log_type_process_stopped(
        &self,
        reason: &'static str,
    ) -> DefaultDiscProcessLogType<ItemType> {
        DefaultDiscProcessLogType::ProcessStopped { reason }
    }

    fn log_type_process_continue(
        &self,
        reason: &'static str,
    ) -> DefaultDiscProcessLogType<ItemType> {
        DefaultDiscProcessLogType::ProcessContinue { reason }
    }

    fn log_type_delay_start(
        &self,
        delay_name: String,
    ) -> DefaultDiscProcessLogType<ItemType> {
        DefaultDiscProcessLogType::DelayStart { delay_name }
    }

    fn log_type_delay_end(
        &self,
        delay_name: String,
    ) -> DefaultDiscProcessLogType<ItemType> {
        DefaultDiscProcessLogType::DelayEnd { delay_name }
    }

    fn log_type_state_change(
        &self,
        new_state: DiscStockState,
    ) -> DefaultDiscProcessLogType<ItemType> {
        DefaultDiscProcessLogType::StateChange { new_state }
    }
}

impl<ItemType> DiscProcess<
        ItemType,
        DiscProcessLog<DefaultDiscProcessLogType<ItemType>, ItemType>,
        DefaultDiscProcessLogType<ItemType>,
    > for DefaultDiscProcess<
        ItemType,
        DiscProcessLog<DefaultDiscProcessLogType<ItemType>, ItemType>,
    >
where
    ItemType: Clone + Debug + Serialize + Send + 'static,
{
    fn req_upstream(&mut self) -> &mut Requestor<(), DiscStockState> {
        &mut self.req_upstream
    }

    fn withdraw_upstream(
        &mut self,
    ) -> &mut Requestor<(usize, EventId), Vec<ItemType>> {
        &mut self.withdraw_upstream
    }

    fn req_downstream(&mut self) -> &mut Requestor<(), DiscStockState> {
        &mut self.req_downstream
    }

    fn push_downstream(
        &mut self,
    ) -> &mut Output<(Vec<ItemType>, EventId)> {
        &mut self.push_downstream
    }

    fn process_quantity_distr(&mut self) -> &mut Distribution {
        &mut self.process_quantity_distr
    }

    fn process_time_distr(&mut self) -> &mut Distribution {
        &mut self.process_time_distr
    }
}

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
    pub req_environment: Requestor<(), BasicEnvironmentState>,
    pub push_downstream: Output<(Vec<ItemType>, EventId)>,
    pub log_emitter: Output<ProcessLog>,

    // Configuration
    pub source_item_generator: Option<Box<dyn Generator<ItemType> + Send>>,
    pub source_quantity_distr: Distribution,
    pub source_time_distr: Distribution,
    pub delay_modes: DelayModes,

    // Runtime state
    pub process_state: Option<(Duration, Vec<ItemType>)>,
    pub env_state: BasicEnvironmentState,

    // Internals
    pub time_to_next_process_event: Option<Duration>,
    pub time_to_next_delay_event: Option<Duration>,
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
            req_environment: Requestor::default(),
            push_downstream: Output::default(),
            log_emitter: Output::default(),

            source_item_generator: None,
            source_quantity_distr: Distribution::default(),
            source_time_distr: Distribution::default(),
            delay_modes: DelayModes::default(),

            process_state: None,
            env_state: BasicEnvironmentState::Normal,

            time_to_next_process_event: None,
            time_to_next_delay_event: None,
            scheduled_event: None,
            next_event_index: 0,
            previous_check_time: MonotonicTime::EPOCH,
        }
    }
}

impl<ItemType, ProcessLog> DefaultDiscSource<ItemType, ProcessLog>
where
    ItemType: Clone + Debug + Serialize + Send + 'static,
    ProcessLog: Clone + Debug + Serialize + Send + 'static,
    Self: DiscProcessCore<ItemType, ProcessLog, DefaultDiscProcessLogType<ItemType>>,
{
    pub fn update_state(
        &mut self,
        mut source_event_id: EventId,
        mut cx: &mut Context<Self>,
    ) -> impl Future<Output = ()> + Send {
        async move {
            self.update_state_since_last_update(&mut source_event_id, &mut cx)
                .await;
            self.update_state_decision_logic(&mut source_event_id, &mut cx)
                .await;
            self.update_state_next_event(&mut source_event_id, &mut cx)
                .await;
        }
    }

    fn update_state_since_last_update(
        &mut self,
        source_event_id: &mut EventId,
        cx: &mut Context<Self>,
    ) -> impl Future<Output = ()> + Send {
        async move {
            if let Some((scheduled_time, _)) = self.scheduled_event() {
                if *scheduled_time <= cx.time() {
                    *self.scheduled_event() = None;
                }
            }

            let duration_since_prev =
                cx.time().duration_since(*self.previous_check_time());

            let is_in_delay = self.delay_modes().active_delay().is_some();
            let is_env_blocked =
                matches!(self.env_state(), BasicEnvironmentState::Stopped);
            let is_in_process =
                self.process_state().is_some() && !is_in_delay && !is_env_blocked;

            if !is_in_delay && !is_env_blocked {
                if let Some((mut time_left, resources)) = self.process_state().take() {
                    time_left = time_left.saturating_sub(duration_since_prev);
                    if time_left.is_zero() {
                        let quantity = resources.len();
                        let log_payload = resources.clone();
                        *source_event_id = self
                            .log(
                                cx.time(),
                                source_event_id.clone(),
                                self.log_type_process_success(quantity, log_payload),
                            )
                            .await;
                        self.push_downstream
                            .send((resources, source_event_id.clone()))
                            .await;
                        *self.time_to_next_process_event() = None;
                    } else {
                        *self.process_state() = Some((time_left, resources));
                        *self.time_to_next_process_event() = Some(time_left);
                    }
                }
            }

            if !is_env_blocked && (is_in_delay || is_in_process) {
                let transition = self.delay_modes().update_state(duration_since_prev);
                if transition.has_changed() {
                    if let Some(delay_name) = &transition.from {
                        *source_event_id = self
                            .log(
                                cx.time(),
                                source_event_id.clone(),
                                self.log_type_delay_end(delay_name.clone()),
                            )
                            .await;
                    }
                    if let Some(delay_name) = &transition.to {
                        *source_event_id = self
                            .log(
                                cx.time(),
                                source_event_id.clone(),
                                self.log_type_delay_start(delay_name.clone()),
                            )
                            .await;
                    }
                }
            }
        }
    }

    fn update_state_decision_logic(
        &mut self,
        source_event_id: &mut EventId,
        cx: &mut Context<Self>,
    ) -> impl Future<Output = ()> + Send {
        async move {
            let time_now = cx.time();

            let new_env_state = match self.req_environment().send(()).await.next() {
                Some(env) => env,
                None => BasicEnvironmentState::Normal,
            };

            match (&self.env_state(), &new_env_state) {
                (BasicEnvironmentState::Normal, BasicEnvironmentState::Stopped) => {
                    *source_event_id = self
                        .log(
                            time_now,
                            source_event_id.clone(),
                            self.log_type_process_stopped("Stopped by environment"),
                        )
                        .await;
                    *self.env_state() = BasicEnvironmentState::Stopped;
                }
                (BasicEnvironmentState::Stopped, BasicEnvironmentState::Normal) => {
                    *source_event_id = self
                        .log(
                            time_now,
                            source_event_id.clone(),
                            self.log_type_process_continue("Resumed by environment"),
                        )
                        .await;
                    *self.env_state() = BasicEnvironmentState::Normal;
                }
                _ => {}
            }

            let is_env_stopped =
                matches!(self.env_state(), BasicEnvironmentState::Stopped);
            let has_active_delay =
                self.delay_modes().active_delay().is_some() || is_env_stopped;

            match (self.process_state(), has_active_delay) {
                (None, false) => {
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
                                    *source_event_id = self
                                        .log(
                                            time_now,
                                            source_event_id.clone(),
                                            self.log_type_process_failure(
                                                "Source item generator not configured",
                                            ),
                                        )
                                        .await;
                                    *self.time_to_next_process_event() = None;
                                    return;
                                }
                            };

                            let mut batch = Vec::with_capacity(requested);
                            batch.resize_with(requested, || generator.clone());
                            if batch.is_empty() {
                                *source_event_id = self
                                    .log(
                                        time_now,
                                        source_event_id.clone(),
                                        self.log_type_process_failure(
                                            "Source produced an empty batch",
                                        ),
                                    )
                                    .await;
                                *self.time_to_next_process_event() = None;
                                return;
                            }

                            *source_event_id = self
                                .log(
                                    time_now,
                                    source_event_id.clone(),
                                    self.log_type_withdraw_request(batch.len()),
                                )
                                .await;

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

                            *source_event_id = self
                                .log(
                                    time_now,
                                    source_event_id.clone(),
                                    self.log_type_process_start(
                                        quantity,
                                        log_payload,
                                    ),
                                )
                                .await;
                            *self.process_state() =
                                Some((process_duration, batch));
                            *self.time_to_next_process_event() =
                                Some(process_duration);
                        }
                        None => {
                            *source_event_id = self
                                .log(
                                    time_now,
                                    source_event_id.clone(),
                                    self.log_type_process_failure(
                                        "Downstream is not connected",
                                    ),
                                )
                                .await;
                            *self.time_to_next_process_event() = None;
                        }
                        Some(DiscStockState::Full { .. }) => {
                            *source_event_id = self
                                .log(
                                    time_now,
                                    source_event_id.clone(),
                                    self.log_type_process_failure("Downstream is full"),
                                )
                                .await;
                            *self.time_to_next_process_event() = None;
                        }
                    }
                }
                (Some((time_left, _)), false) => {
                    *self.time_to_next_process_event() = Some(*time_left);
                }
                (_, true) => {
                    *self.time_to_next_process_event() = self
                        .delay_modes()
                        .active_delay()
                        .map(|(_, delay_state)| *delay_state);
                }
            }
        }
    }

    fn update_state_next_event(
        &mut self,
        source_event_id: &mut EventId,
        cx: &mut Context<Self>,
    ) -> impl Future<Output = ()> + Send {
        async move {
            let is_env_stopped =
                matches!(self.env_state(), BasicEnvironmentState::Stopped);
            let has_active_delay =
                self.delay_modes().active_delay().is_some() || is_env_stopped;

            if self.process_state().is_some()
                || has_active_delay
                || !is_env_stopped
            {
                *self.time_to_next_delay_event() = self
                    .delay_modes()
                    .get_next_event()
                    .map(|(_, delay_state)| delay_state.as_duration());
            } else {
                *self.time_to_next_delay_event() = None;
            }

            let next_event = [
                self.time_to_next_delay_event().clone(),
                self.time_to_next_process_event().clone(),
            ]
            .into_iter()
            .flatten()
            .min();

            println!(
                "### [{}] Next event in: {:?}",
                self.element_name,
                next_event
            );

            if let Some(time_until_next) = next_event {
                if time_until_next.is_zero() {
                    panic!("Time until next event is zero!");
                }

                let next_time = cx.time() + time_until_next;

                if let Some((scheduled_time, action_key)) =
                    self.scheduled_event().take()
                {
                    if next_time < scheduled_time {
                        action_key.cancel();
                        let new_key = cx
                            .schedule_keyed_event(
                                next_time,
                                Self::update_state,
                                source_event_id.clone(),
                            )
                            .unwrap();
                        *self.scheduled_event() = Some((next_time, new_key));
                    } else {
                        *self.scheduled_event() =
                            Some((scheduled_time, action_key));
                    }
                } else {
                    let new_key = cx
                        .schedule_keyed_event(
                            next_time,
                            Self::update_state,
                            source_event_id.clone(),
                        )
                        .unwrap();
                    *self.scheduled_event() = Some((next_time, new_key));
                }
            }

            *self.previous_check_time() = cx.time();
        }
    }
}

impl<ItemType: Clone + Debug + Serialize + Send + 'static> Model for DefaultDiscSource<
    ItemType, 
    DiscProcessLog<DefaultDiscProcessLogType<ItemType>, ItemType>
>
where
    DiscProcessLog<DefaultDiscProcessLogType<ItemType>, ItemType>: Serialize,
    Self: ToLogRecord<
        DefaultDiscProcessLogType<ItemType>,
        DiscProcessLog<DefaultDiscProcessLogType<ItemType>, ItemType>
    > {
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

impl<ItemType> ToLogRecord<
        DefaultDiscProcessLogType<ItemType>,
        DiscProcessLog<DefaultDiscProcessLogType<ItemType>, ItemType>,
    > for DefaultDiscSource<
        ItemType,
        DiscProcessLog<DefaultDiscProcessLogType<ItemType>, ItemType>,
    >
where
    ItemType: Clone + Debug + Serialize + Send + 'static,
{
    fn to_record(
        &mut self,
        now: MonotonicTime,
        source_event_id: EventId,
        event_id: EventId,
        details: DefaultDiscProcessLogType<ItemType>,
    ) -> DiscProcessLog<DefaultDiscProcessLogType<ItemType>, ItemType> {
        DiscProcessLog {
            time: now.to_chrono_date_time(0).unwrap().to_string(),
            event_id,
            source_event_id,
            element_name: self.element_name.clone(),
            element_type: self.element_type.clone(),
            details,
            phantom: std::marker::PhantomData,
        }
    }
}

impl<ItemType> DiscProcessCore<
        ItemType,
        DiscProcessLog<DefaultDiscProcessLogType<ItemType>, ItemType>,
        DefaultDiscProcessLogType<ItemType>,
    > for DefaultDiscSource<
        ItemType,
        DiscProcessLog<DefaultDiscProcessLogType<ItemType>, ItemType>,
    >
where
    ItemType: Clone + Debug + Serialize + Send + 'static,
{
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

    fn log_emitter(
        &mut self,
    ) -> &mut Output<
        DiscProcessLog<DefaultDiscProcessLogType<ItemType>, ItemType>,
    > {
        &mut self.log_emitter
    }

    fn scheduled_event(
        &mut self,
    ) -> &mut Option<(MonotonicTime, ActionKey)> {
        &mut self.scheduled_event
    }

    fn previous_check_time(&mut self) -> &mut MonotonicTime {
        &mut self.previous_check_time
    }

    fn delay_modes(&mut self) -> &mut DelayModes {
        &mut self.delay_modes
    }

    fn process_state(
        &mut self,
    ) -> &mut Option<(Duration, Vec<ItemType>)> {
        &mut self.process_state
    }

    fn env_state(&mut self) -> &mut BasicEnvironmentState {
        &mut self.env_state
    }

    fn req_environment(
        &mut self,
    ) -> &mut Requestor<(), BasicEnvironmentState> {
        &mut self.req_environment
    }

    fn time_to_next_process_event(
        &mut self,
    ) -> &mut Option<Duration> {
        &mut self.time_to_next_process_event
    }

    fn time_to_next_delay_event(
        &mut self,
    ) -> &mut Option<Duration> {
        &mut self.time_to_next_delay_event
    }

    fn log_type_withdraw_request(&self, quantity: usize) -> DefaultDiscProcessLogType<ItemType> {
        DefaultDiscProcessLogType::WithdrawRequest { quantity }
    }

    fn log_type_process_start(
        &self,
        quantity: usize,
        resources: Vec<ItemType>,
    ) -> DefaultDiscProcessLogType<ItemType> {
        DefaultDiscProcessLogType::ProcessStart { quantity, resources }
    }

    fn log_type_process_success(
        &self,
        quantity: usize,
        resources: Vec<ItemType>,
    ) -> DefaultDiscProcessLogType<ItemType> {
        DefaultDiscProcessLogType::ProcessSuccess { quantity, resources }
    }

    fn log_type_process_failure(
        &self,
        reason: &'static str,
    ) -> DefaultDiscProcessLogType<ItemType> {
        DefaultDiscProcessLogType::ProcessFailure { reason }
    }

    fn log_type_process_stopped(
        &self,
        reason: &'static str,
    ) -> DefaultDiscProcessLogType<ItemType> {
        DefaultDiscProcessLogType::ProcessStopped { reason }
    }

    fn log_type_process_continue(
        &self,
        reason: &'static str,
    ) -> DefaultDiscProcessLogType<ItemType> {
        DefaultDiscProcessLogType::ProcessContinue { reason }
    }

    fn log_type_delay_start(
        &self,
        delay_name: String,
    ) -> DefaultDiscProcessLogType<ItemType> {
        DefaultDiscProcessLogType::DelayStart { delay_name }
    }

    fn log_type_delay_end(
        &self,
        delay_name: String,
    ) -> DefaultDiscProcessLogType<ItemType> {
        DefaultDiscProcessLogType::DelayEnd { delay_name }
    }

    fn log_type_state_change(
        &self,
        new_state: DiscStockState,
    ) -> DefaultDiscProcessLogType<ItemType> {
        DefaultDiscProcessLogType::StateChange { new_state }
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
    pub req_environment: Requestor<(), BasicEnvironmentState>,
    pub withdraw_upstream: Requestor<(usize, EventId), Vec<ItemType>>,
    pub log_emitter: Output<ProcessLog>,

    // Configuration
    pub sink_quantity_distr: Distribution,
    pub sink_time_distr: Distribution,
    pub delay_modes: DelayModes,

    // Runtime state
    pub process_state: Option<(Duration, Vec<ItemType>)>,
    pub env_state: BasicEnvironmentState,

    // Internals
    pub time_to_next_process_event: Option<Duration>,
    pub time_to_next_delay_event: Option<Duration>,
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
            req_environment: Requestor::default(),
            withdraw_upstream: Requestor::default(),
            log_emitter: Output::default(),

            sink_quantity_distr: Distribution::default(),
            sink_time_distr: Distribution::default(),
            delay_modes: DelayModes::default(),

            process_state: None,
            env_state: BasicEnvironmentState::Normal,

            time_to_next_process_event: None,
            time_to_next_delay_event: None,
            scheduled_event: None,
            next_event_index: 0,
            previous_check_time: MonotonicTime::EPOCH,
        }
    }
}

impl<ItemType, ProcessLog> DefaultDiscSink<ItemType, ProcessLog>
where
    ItemType: Clone + Debug + Serialize + Send + 'static,
    ProcessLog: Clone + Debug + Serialize + Send + 'static,
    Self: DiscProcessCore<ItemType, ProcessLog, DefaultDiscProcessLogType<ItemType>>,
{
    pub fn update_state(
        &mut self,
        mut source_event_id: EventId,
        mut cx: &mut Context<Self>,
    ) -> impl Future<Output = ()> + Send {
        async move {
            self.update_state_since_last_update(&mut source_event_id, &mut cx)
                .await;
            self.update_state_decision_logic(&mut source_event_id, &mut cx)
                .await;
            self.update_state_next_event(&mut source_event_id, &mut cx)
                .await;
        }
    }

    fn update_state_since_last_update(
        &mut self,
        source_event_id: &mut EventId,
        cx: &mut Context<Self>,
    ) -> impl Future<Output = ()> + Send {
        async move {
            if let Some((scheduled_time, _)) = self.scheduled_event() {
                if *scheduled_time <= cx.time() {
                    *self.scheduled_event() = None;
                }
            }

            let duration_since_prev =
                cx.time().duration_since(*self.previous_check_time());

            let is_in_delay = self.delay_modes().active_delay().is_some();
            let is_env_blocked =
                matches!(self.env_state(), BasicEnvironmentState::Stopped);
            let is_in_process =
                self.process_state().is_some() && !is_in_delay && !is_env_blocked;

            if !is_in_delay && !is_env_blocked {
                if let Some((mut time_left, mut resources)) = self.process_state().take() {
                    time_left = time_left.saturating_sub(duration_since_prev);
                    if time_left.is_zero() {
                        let quantity = resources.len();
                        let log_payload = resources.clone();
                        *source_event_id = self
                            .log(
                                cx.time(),
                                source_event_id.clone(),
                                self.log_type_process_success(quantity, log_payload),
                            )
                            .await;
                        resources.clear();
                        *self.time_to_next_process_event() = None;
                    } else {
                        *self.process_state() = Some((time_left, resources));
                        *self.time_to_next_process_event() = Some(time_left);
                    }
                }
            }

            if !is_env_blocked && (is_in_delay || is_in_process) {
                let transition = self.delay_modes().update_state(duration_since_prev);
                if transition.has_changed() {
                    if let Some(delay_name) = &transition.from {
                        *source_event_id = self
                            .log(
                                cx.time(),
                                source_event_id.clone(),
                                self.log_type_delay_end(delay_name.clone()),
                            )
                            .await;
                    }
                    if let Some(delay_name) = &transition.to {
                        *source_event_id = self
                            .log(
                                cx.time(),
                                source_event_id.clone(),
                                self.log_type_delay_start(delay_name.clone()),
                            )
                            .await;
                    }
                }
            }
        }
    }

    fn update_state_decision_logic(
        &mut self,
        source_event_id: &mut EventId,
        cx: &mut Context<Self>,
    ) -> impl Future<Output = ()> + Send {
        async move {
            let time_now = cx.time();

            let new_env_state = match self.req_environment().send(()).await.next() {
                Some(env) => env,
                None => BasicEnvironmentState::Normal,
            };

            match (&self.env_state(), &new_env_state) {
                (BasicEnvironmentState::Normal, BasicEnvironmentState::Stopped) => {
                    *source_event_id = self
                        .log(
                            time_now,
                            source_event_id.clone(),
                            self.log_type_process_stopped("Stopped by environment"),
                        )
                        .await;
                    *self.env_state() = BasicEnvironmentState::Stopped;
                }
                (BasicEnvironmentState::Stopped, BasicEnvironmentState::Normal) => {
                    *source_event_id = self
                        .log(
                            time_now,
                            source_event_id.clone(),
                            self.log_type_process_continue("Resumed by environment"),
                        )
                        .await;
                    *self.env_state() = BasicEnvironmentState::Normal;
                }
                _ => {}
            }

            let is_env_stopped =
                matches!(self.env_state(), BasicEnvironmentState::Stopped);
            let has_active_delay =
                self.delay_modes().active_delay().is_some() || is_env_stopped;

            match (self.process_state(), has_active_delay) {
                (None, false) => {
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

                            *source_event_id = self
                                .log(
                                    time_now,
                                    source_event_id.clone(),
                                    self.log_type_withdraw_request(requested),
                                )
                                .await;

                            let pulled = self
                                .withdraw_upstream
                                .send((requested, source_event_id.clone()))
                                .await
                                .next();

                            match pulled {
                                Some(mut batch) => {
                                    if batch.is_empty() {
                                        *source_event_id = self
                                            .log(
                                                time_now,
                                                source_event_id.clone(),
                                                self.log_type_process_failure(
                                                    "Upstream returned no items",
                                                ),
                                            )
                                            .await;
                                        *self.time_to_next_process_event() = None;
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

                                    *source_event_id = self
                                        .log(
                                            time_now,
                                            source_event_id.clone(),
                                            self.log_type_process_start(
                                                quantity,
                                                log_payload,
                                            ),
                                        )
                                        .await;
                                    *self.process_state() =
                                        Some((process_duration, batch));
                                    *self.time_to_next_process_event() =
                                        Some(process_duration);
                                }
                                None => {
                                    *source_event_id = self
                                        .log(
                                            time_now,
                                            source_event_id.clone(),
                                            self.log_type_process_failure(
                                                "Upstream requestor closed",
                                            ),
                                        )
                                        .await;
                                    *self.time_to_next_process_event() = None;
                                }
                            }
                        }
                        Some(DiscStockState::Empty { .. }) => {
                            *source_event_id = self
                                .log(
                                    time_now,
                                    source_event_id.clone(),
                                    self.log_type_process_failure("Upstream is empty"),
                                )
                                .await;
                            *self.time_to_next_process_event() = None;
                        }
                        None => {
                            *source_event_id = self
                                .log(
                                    time_now,
                                    source_event_id.clone(),
                                    self.log_type_process_failure(
                                        "Upstream is not connected",
                                    ),
                                )
                                .await;
                            *self.time_to_next_process_event() = None;
                        }
                    }
                }
                (Some((time_left, _)), false) => {
                    *self.time_to_next_process_event() = Some(*time_left);
                }
                (_, true) => {
                    *self.time_to_next_process_event() = self
                        .delay_modes()
                        .active_delay()
                        .map(|(_, delay_state)| *delay_state);
                }
            }
        }
    }

    fn update_state_next_event(
        &mut self,
        source_event_id: &mut EventId,
        cx: &mut Context<Self>,
    ) -> impl Future<Output = ()> + Send {
        async move {
            let is_env_stopped =
                matches!(self.env_state(), BasicEnvironmentState::Stopped);
            let has_active_delay =
                self.delay_modes().active_delay().is_some() || is_env_stopped;

            if self.process_state().is_some()
                || has_active_delay
                || !is_env_stopped
            {
                *self.time_to_next_delay_event() = self
                    .delay_modes()
                    .get_next_event()
                    .map(|(_, delay_state)| delay_state.as_duration());
            } else {
                *self.time_to_next_delay_event() = None;
            }

            let next_event = [
                self.time_to_next_delay_event().clone(),
                self.time_to_next_process_event().clone(),
            ]
            .into_iter()
            .flatten()
            .min();

            if let Some(time_until_next) = next_event {
                if time_until_next.is_zero() {
                    panic!("Time until next event is zero!");
                }

                let next_time = cx.time() + time_until_next;

                if let Some((scheduled_time, action_key)) =
                    self.scheduled_event().take()
                {
                    if next_time < scheduled_time {
                        action_key.cancel();
                        let new_key = cx
                            .schedule_keyed_event(
                                next_time,
                                Self::update_state,
                                source_event_id.clone(),
                            )
                            .unwrap();
                        *self.scheduled_event() = Some((next_time, new_key));
                    } else {
                        *self.scheduled_event() =
                            Some((scheduled_time, action_key));
                    }
                } else {
                    let new_key = cx
                        .schedule_keyed_event(
                            next_time,
                            Self::update_state,
                            source_event_id.clone(),
                        )
                        .unwrap();
                    *self.scheduled_event() = Some((next_time, new_key));
                }
            }

            *self.previous_check_time() = cx.time();
        }
    }
}

impl<ItemType: Clone + Debug + Serialize + Send + 'static> Model for DefaultDiscSink<
    ItemType, 
    DiscProcessLog<DefaultDiscProcessLogType<ItemType>, ItemType>
>
where
    DiscProcessLog<DefaultDiscProcessLogType<ItemType>, ItemType>: Serialize,
    Self: ToLogRecord<
        DefaultDiscProcessLogType<ItemType>,
        DiscProcessLog<DefaultDiscProcessLogType<ItemType>, ItemType>
    > {
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

impl<ItemType> ToLogRecord<
        DefaultDiscProcessLogType<ItemType>,
        DiscProcessLog<DefaultDiscProcessLogType<ItemType>, ItemType>,
    > for DefaultDiscSink<
        ItemType,
        DiscProcessLog<DefaultDiscProcessLogType<ItemType>, ItemType>,
    >
where
    ItemType: Clone + Debug + Serialize + Send + 'static,
{
    fn to_record(
        &mut self,
        now: MonotonicTime,
        source_event_id: EventId,
        event_id: EventId,
        details: DefaultDiscProcessLogType<ItemType>,
    ) -> DiscProcessLog<DefaultDiscProcessLogType<ItemType>, ItemType> {
        DiscProcessLog {
            time: now.to_chrono_date_time(0).unwrap().to_string(),
            event_id,
            source_event_id,
            element_name: self.element_name.clone(),
            element_type: self.element_type.clone(),
            details,
            phantom: std::marker::PhantomData,
        }
    }
}

impl<ItemType> DiscProcessCore<
        ItemType,
        DiscProcessLog<DefaultDiscProcessLogType<ItemType>, ItemType>,
        DefaultDiscProcessLogType<ItemType>,
    > for DefaultDiscSink<
        ItemType,
        DiscProcessLog<DefaultDiscProcessLogType<ItemType>, ItemType>,
    >
where
    ItemType: Clone + Debug + Serialize + Send + 'static,
{
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

    fn log_emitter(
        &mut self,
    ) -> &mut Output<
        DiscProcessLog<DefaultDiscProcessLogType<ItemType>, ItemType>,
    > {
        &mut self.log_emitter
    }

    fn scheduled_event(
        &mut self,
    ) -> &mut Option<(MonotonicTime, ActionKey)> {
        &mut self.scheduled_event
    }

    fn previous_check_time(&mut self) -> &mut MonotonicTime {
        &mut self.previous_check_time
    }

    fn delay_modes(&mut self) -> &mut DelayModes {
        &mut self.delay_modes
    }

    fn process_state(
        &mut self,
    ) -> &mut Option<(Duration, Vec<ItemType>)> {
        &mut self.process_state
    }

    fn env_state(&mut self) -> &mut BasicEnvironmentState {
        &mut self.env_state
    }

    fn req_environment(
        &mut self,
    ) -> &mut Requestor<(), BasicEnvironmentState> {
        &mut self.req_environment
    }

    fn time_to_next_process_event(
        &mut self,
    ) -> &mut Option<Duration> {
        &mut self.time_to_next_process_event
    }

    fn time_to_next_delay_event(
        &mut self,
    ) -> &mut Option<Duration> {
        &mut self.time_to_next_delay_event
    }

    fn log_type_withdraw_request(&self, quantity: usize) -> DefaultDiscProcessLogType<ItemType> {
        DefaultDiscProcessLogType::WithdrawRequest { quantity }
    }

    fn log_type_process_start(
        &self,
        quantity: usize,
        resources: Vec<ItemType>,
    ) -> DefaultDiscProcessLogType<ItemType> {
        DefaultDiscProcessLogType::ProcessStart { quantity, resources }
    }

    fn log_type_process_success(
        &self,
        quantity: usize,
        resources: Vec<ItemType>,
    ) -> DefaultDiscProcessLogType<ItemType> {
        DefaultDiscProcessLogType::ProcessSuccess { quantity, resources }
    }

    fn log_type_process_failure(
        &self,
        reason: &'static str,
    ) -> DefaultDiscProcessLogType<ItemType> {
        DefaultDiscProcessLogType::ProcessFailure { reason }
    }

    fn log_type_process_stopped(
        &self,
        reason: &'static str,
    ) -> DefaultDiscProcessLogType<ItemType> {
        DefaultDiscProcessLogType::ProcessStopped { reason }
    }

    fn log_type_process_continue(
        &self,
        reason: &'static str,
    ) -> DefaultDiscProcessLogType<ItemType> {
        DefaultDiscProcessLogType::ProcessContinue { reason }
    }

    fn log_type_delay_start(
        &self,
        delay_name: String,
    ) -> DefaultDiscProcessLogType<ItemType> {
        DefaultDiscProcessLogType::DelayStart { delay_name }
    }

    fn log_type_delay_end(
        &self,
        delay_name: String,
    ) -> DefaultDiscProcessLogType<ItemType> {
        DefaultDiscProcessLogType::DelayEnd { delay_name }
    }

    fn log_type_state_change(
        &self,
        new_state: DiscStockState,
    ) -> DefaultDiscProcessLogType<ItemType> {
        DefaultDiscProcessLogType::StateChange { new_state }
    }
}