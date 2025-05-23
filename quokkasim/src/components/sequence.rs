use serde::ser::SerializeStruct;
use serde::Serialize;

use crate::prelude::*;
use std::collections::VecDeque;
use std::time::Duration;
use std::fmt::Debug;

#[derive(Debug, Clone)]
pub enum SequenceStockState {
    Empty { occupied: u32, empty: u32 },
    Normal { occupied: u32, empty: u32 },
    Full { occupied: u32, empty: u32 },
}

impl SequenceStockState {
    pub fn get_name(&self) -> String {
        match self {
            SequenceStockState::Empty { .. } => "Empty".to_string(),
            SequenceStockState::Normal { .. } => "Normal".to_string(),
            SequenceStockState::Full { .. } => "Full".to_string(),
        }
    }
}

impl StateEq for SequenceStockState {
    fn is_same_state(&self, other: &Self) -> bool {
        match (self, other) {
            (SequenceStockState::Empty { .. }, SequenceStockState::Empty { ..  }) => true,
            (SequenceStockState::Normal { .. }, SequenceStockState::Normal { .. }) => true,
            (SequenceStockState::Full { .. }, SequenceStockState::Full { .. }) => true,
            _ => false,
        }
    }
}

pub struct SequenceStock<T> where T: Clone + Default + Send + 'static {
    pub element_name: String,
    pub element_type: String,
    pub sequence: SeqDeque<T>,
    pub log_emitter: Output<SequenceStockLog<T>>,
    pub state_emitter: Output<NotificationMetadata>,
    pub low_capacity: u32,
    pub max_capacity: u32,
    pub prev_state: Option<SequenceStockState>,
    next_event_id: u64,
}
impl<T: Clone + Default + Send + 'static> Default for SequenceStock<T> {
    fn default() -> Self {
        SequenceStock {
            element_name: "SequenceStock".to_string(),
            element_type: "SequenceStock".to_string(),
            sequence: SeqDeque::default(),
            log_emitter: Output::new(),
            state_emitter: Output::new(),
            low_capacity: 0,
            max_capacity: 1,
            prev_state: None,
            next_event_id: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SeqDeque<TT> {
    pub deque: VecDeque<TT>,
}

impl<TT> VectorArithmetic<Option<TT>, (), u32> for SeqDeque<TT> {
    fn add(&mut self, other: Option<TT>) {
        match other { 
            Some(item) => {
                self.deque.push_back(item);
            },
            _ => {}
        };
    }
    fn subtract_parts(&self, _: ()) -> SubtractParts<Self, Option<TT>> {
        todo!()
    }
    fn total(&self) -> u32 {
        self.deque.len() as u32
    }
}

impl<TT: Default> Default for SeqDeque<TT> {
    fn default() -> Self {
        SeqDeque {
            deque: VecDeque::new(),
        }
    }
}

impl<T: Clone + Debug + Default + Send> Stock<SeqDeque<T>, Option<T>, (), Option<T>, u32> for SequenceStock<T> where Self: Model {
    type StockState = SequenceStockState;
    fn get_state(&mut self) -> Self::StockState {
        let occupied = self.sequence.total();
        let empty = self.max_capacity.saturating_sub(occupied); // If occupied beyond capacity, just say no empty space
        if self.sequence.total() <= self.low_capacity {
            SequenceStockState::Empty { occupied, empty }
        } else if self.sequence.total() >= self.max_capacity {
            SequenceStockState::Full { occupied, empty }
        } else {
            SequenceStockState::Normal { occupied, empty }
        }
    }
    fn get_previous_state(&mut self) -> &Option<Self::StockState> {
        &self.prev_state
    }
    fn set_previous_state(&mut self) {
        self.prev_state = Some(self.get_state());
    }
    fn get_resource(&self) -> &SeqDeque<T> {
        &self.sequence
    }
    fn add_impl<'a>(&'a mut self, payload: &'a (Option<T>, NotificationMetadata), cx: &'a mut Context<Self>) -> impl Future<Output = ()> + 'a {
        async move {
            self.prev_state = Some(self.get_state());
            match payload.0 {
                Some(ref item) => {
                    self.sequence.deque.push_back(item.clone());
                }
                None => {}
            }
        }
    }
    fn remove_impl<'a>(&'a mut self, payload: &'a ((), NotificationMetadata), cx: &'a mut Context<Self>) -> impl Future<Output = Option<T>> + 'a {
        async move {
            self.prev_state = Some(self.get_state());
            self.sequence.deque.pop_front()
        }
    }

    fn emit_change<'a>(&'a mut self, payload: NotificationMetadata, cx: &'a mut nexosim::model::Context<Self>) -> impl Future<Output = ()> + Send + 'a {
        async move {
            self.state_emitter.send(payload).await;
            self.log(cx.time(), "Emit Change".to_string()).await;
        }
    }

    fn log(&mut self, time: MonotonicTime, log_type: String) -> impl Future<Output = ()> + Send {
        async move {
            let log = SequenceStockLog {
                time: time.to_chrono_date_time(0).unwrap().to_string(),
                event_id: self.next_event_id,
                element_name: self.element_name.clone(),
                element_type: self.element_type.clone(),
                log_type,
                state: self.get_state(),
                sequence: self.sequence.clone(),
            };
            self.log_emitter.send(log).await;
            self.next_event_id += 1;
        }
    }
}

impl<T: Clone + Default + Send> Model for SequenceStock<T> {}

impl<T: Clone + Default + Debug + Send> SequenceStock<T> {
    pub fn new() -> Self {
        SequenceStock::default()
    }
    pub fn with_name(mut self, name: String) -> Self {
        self.element_name = name;
        self
    }
    pub fn with_type(mut self, type_: String) -> Self {
        self.element_type = type_;
        self
    }

    pub fn with_initial_contents(mut self, contents: Vec<T>) -> Self {
        self.sequence.deque = contents.into_iter().collect();
        self
    }

    pub fn with_low_capacity(mut self, low_capacity: u32) -> Self {
        self.low_capacity = low_capacity;
        self
    }

    pub fn with_max_capacity(mut self, max_capacity: u32) -> Self {
        self.max_capacity = max_capacity;
        self
    }
}

pub struct SequenceStockLogger<T> where T: Send {
    pub name: String,
    pub buffer: EventQueue<SequenceStockLog<T>>,
}

impl<T> Logger for SequenceStockLogger<T> where SequenceStockLog<T>: Serialize, T: Send + 'static {
    type RecordType = SequenceStockLog<T>;
    fn get_name(&self) -> &String {
        &self.name
    }
    fn get_buffer(self) -> EventQueue<Self::RecordType> {
        self.buffer
    }
    fn new(name: String) -> Self {
        SequenceStockLogger {
            name,
            buffer: EventQueue::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SequenceStockLog<T> {
    pub time: String,
    pub event_id: u64,
    pub element_name: String,
    pub element_type: String,
    pub log_type: String,
    pub state: SequenceStockState,
    pub sequence: SeqDeque<T>,
}

impl Serialize for SequenceStockLog<String> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer {
        let mut state = serializer.serialize_struct("SequenceStockLog", 6)?;
        state.serialize_field("time", &self.time)?;
        state.serialize_field("event_id", &self.event_id)?;
        state.serialize_field("element_name", &self.element_name)?;
        state.serialize_field("element_type", &self.element_type)?;
        state.serialize_field("log_type", &self.log_type)?;
        state.serialize_field("state", &self.state.get_name())?;
        state.serialize_field("sequence", &format!("{:?}", self.sequence.deque))?;
        state.end()
    }
}

/**
 * U: Parameter type pushed to downstream stock
 * V: Parameter type requested from upstream stock
 * W: Parameter type received from upstream stock
 */
pub struct SequenceProcess<U: Clone + Send + 'static, V: Clone + Send + 'static, W: Clone + Send + 'static> where  {
    pub element_name: String,
    pub element_type: String,
    pub req_upstream: Requestor<(), SequenceStockState>,
    pub req_downstream: Requestor<(), SequenceStockState>,
    pub withdraw_upstream: Requestor<(V, NotificationMetadata), W>,
    pub push_downstream: Output<(U, NotificationMetadata)>,
    pub process_state: Option<(Duration, W)>,
    pub process_time_distr: Option<Distribution>,
    pub process_quantity_distr: Option<Distribution>,
    pub log_emitter: Output<SequenceProcessLog<U>>,
    time_to_next_event: Option<Duration>,
    next_event_id: u64,
    pub previous_check_time: MonotonicTime,
}
impl<U: Clone + Send + 'static, V: Clone + Send + 'static, W: Clone + Send + 'static> Default for SequenceProcess<U, V, W> {
    fn default() -> Self {
        SequenceProcess {
            element_name: "SequenceProcess".to_string(),
            element_type: "SequenceProcess".to_string(),
            req_upstream: Requestor::new(),
            req_downstream: Requestor::new(),
            withdraw_upstream: Requestor::new(),
            push_downstream: Output::new(),
            process_state: None,
            process_time_distr: None,
            process_quantity_distr: None,
            log_emitter: Output::new(),
            time_to_next_event: None,
            next_event_id: 0,
            previous_check_time: MonotonicTime::EPOCH,
        }
    }
}

impl<U: Clone + Send + 'static, V: Clone + Send + 'static, W: Clone + Send + 'static> SequenceProcess<U, V, W> {
    pub fn new() -> Self {
        SequenceProcess::default()
    }
    pub fn with_name(mut self, name: String) -> Self {
        self.element_name = name;
        self
    }
    pub fn with_type(mut self, type_: String) -> Self {
        self.element_type = type_;
        self
    }

    pub fn with_process_time_distr(mut self, distr: Distribution) -> Self {
        self.process_time_distr = Some(distr);
        self
    }
}


impl<U: Clone + Send + 'static, V: Clone + Send + 'static, W: Clone + Send + 'static> Model for SequenceProcess<U, V, W> {}

impl<U: Clone + Debug + Send + 'static> Process<SeqDeque<U>, Option<U>, (), u32> for SequenceProcess<Option<U>, (), Option<U>>
where
    Self: Model,
    SeqDeque<U>: VectorArithmetic<Option<U>, (), u32>,
{
    type LogDetailsType = SequenceProcessLogType<Option<U>>;

    fn get_time_to_next_event(&mut self) -> &Option<Duration> {
        &self.time_to_next_event
    }

    fn set_time_to_next_event(&mut self, time: Option<Duration>) {
        self.time_to_next_event = time;
    }

    fn set_previous_check_time(&mut self, time: MonotonicTime) {
        self.previous_check_time = time;
    }

    fn update_state_impl<'a>(&'a mut self, notif_meta: &NotificationMetadata, cx: &'a mut Context<Self>) -> impl Future<Output = ()> + 'a where Self: Model {
        async move {
            let time = cx.time();

            match self.process_state.take() {
                Some((mut process_time_left, resource)) => {
                    let duration_since_prev_check = cx.time().duration_since(self.previous_check_time);
                    process_time_left = process_time_left.saturating_sub(duration_since_prev_check);
                    if process_time_left.is_zero() {
                        self.log(time, SequenceProcessLogType::ProcessSuccess { resource: resource.clone() }).await;
                        self.push_downstream.send((resource.clone(), NotificationMetadata {
                            time,
                            element_from: self.element_name.clone(),
                            message: "ProcessStart".into(),
                        })).await;
                    } else {
                        self.process_state = Some((process_time_left, resource));
                    }
                }
                None => {}
            }
            match self.process_state {
                None => {
                    let us_state = self.req_upstream.send(()).await.next();
                    let ds_state = self.req_downstream.send(()).await.next();
                    match (&us_state, &ds_state) {
                        (
                            Some(SequenceStockState::Normal { .. } | SequenceStockState::Full { .. }),
                            Some(SequenceStockState::Empty { .. } | SequenceStockState::Normal { .. }),
                        ) => {
                            let moved = self.withdraw_upstream.send(((), NotificationMetadata {
                                time,
                                element_from: self.element_name.clone(),
                                message: "Withdraw request".into(),
                            })).await.next().unwrap();
                            let process_duration_secs = self.process_time_distr.as_mut().unwrap_or_else(|| {
                                panic!("Process time distribution not set for process {}", self.element_name);
                            }).sample();
                            self.process_state = Some((Duration::from_secs_f64(process_duration_secs.clone()), moved.clone()));
                            self.log(time, SequenceProcessLogType::ProcessStart { resource: moved.clone() }).await;
                            self.time_to_next_event = Some(Duration::from_secs_f64(process_duration_secs));
                        },
                        (Some(SequenceStockState::Empty { .. }), _ ) => {
                            self.log(time, SequenceProcessLogType::ProcessFailure { reason: "Upstream is empty" }).await;
                            self.time_to_next_event = None;
                        },
                        (None, _) => {
                            self.log(time, SequenceProcessLogType::ProcessFailure { reason: "Upstream is not connected" }).await;
                            self.time_to_next_event = None;
                        },
                        (_, Some(SequenceStockState::Full { .. })) => {
                            self.log(time, SequenceProcessLogType::ProcessFailure { reason: "Downstream is full" }).await;
                            self.time_to_next_event = None;
                        },
                        (_, None) => {
                            self.log(time, SequenceProcessLogType::ProcessFailure { reason: "Downstream is not connected" }).await;
                            self.time_to_next_event = None;
                        }
                    }
                },
                Some((time, _)) => {
                    self.time_to_next_event = Some(time);
                }
            }
        }
    }

    fn post_update_state<'a> (&'a mut self, notif_meta: &'a NotificationMetadata, cx: &'a mut Context<Self>) -> impl Future<Output = ()> + Send + 'a where Self: Model {
        async move {
            self.set_previous_check_time(cx.time());
            match self.time_to_next_event {
                None => {},
                Some(time_until_next) => {
                    if time_until_next.is_zero() {
                        panic!("Time until next event is zero!");
                    } else {
                        let next_time = cx.time() + time_until_next;
                        cx.schedule_event(next_time, <Self as Process<SeqDeque<U>, Option<U>, (), u32>>::update_state, notif_meta.clone()).unwrap();
                    };
                }
            };
        }
    }

    fn log<'a>(&'a mut self, time: MonotonicTime, details: SequenceProcessLogType<Option<U>>) -> impl Future<Output = ()> + Send {
        async move {
            let log = SequenceProcessLog {
                time: time.to_chrono_date_time(0).unwrap().to_string(),
                event_id: self.next_event_id,
                element_name: self.element_name.clone(),
                element_type: self.element_type.clone(),
                event: details,
            };
            self.next_event_id += 1;
            self.log_emitter.send(log).await;
        }
    }
}

#[derive(Debug, Clone)]
pub enum SequenceProcessLogType<T> {
    ProcessStart { resource: T },
    ProcessSuccess { resource: T },
    ProcessFailure { reason: &'static str },
}


#[derive(Debug, Clone)]
pub struct SequenceProcessLog<T> {
    pub time: String,
    pub event_id: u64,
    pub element_name: String,
    pub element_type: String,
    pub event: SequenceProcessLogType<T>,
}

impl Serialize for SequenceProcessLog<Option<String>> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer {
        let mut state = serializer.serialize_struct("SequenceProcessLog", 7)?;
        state.serialize_field("time", &self.time)?;
        state.serialize_field("event_id", &self.event_id)?;
        state.serialize_field("element_name", &self.element_name)?;
        state.serialize_field("element_type", &self.element_type)?;
        let (event_type, item, reason): (String, Option<&str>, Option<&str>) = match &self.event {
            SequenceProcessLogType::ProcessStart { resource } => ("ProcessStart".into(), resource.as_deref(), None),
            SequenceProcessLogType::ProcessSuccess { resource } => ("ProcessSuccess".into(), resource.as_deref(), None),
            SequenceProcessLogType::ProcessFailure { reason } => ("ProcessFailure".into(), None, Some(reason)),
        };
        state.serialize_field("event_type", &event_type)?;
        state.serialize_field("item", &item)?;
        state.serialize_field("reason", &reason)?;
        state.end()
    }
}

pub struct SequenceProcessLogger<T> where T: Send {
    pub name: String,
    pub buffer: EventQueue<SequenceProcessLog<T>>,
}

impl<T> Logger for SequenceProcessLogger<T> where SequenceProcessLog<T>: Serialize, T: Send + 'static {
    type RecordType = SequenceProcessLog<T>;
    fn get_name(&self) -> &String {
        &self.name
    }
    fn get_buffer(self) -> EventQueue<Self::RecordType> {
        self.buffer
    }
    fn new(name: String) -> Self {
        SequenceProcessLogger {
            name,
            buffer: EventQueue::new(),
        }
    }
}

