use serde::ser::SerializeStruct;
use serde::Serialize;

use crate::prelude::*;
use std::collections::{VecDeque, HashMap};
use std::time::Duration;
use std::fmt::Debug;

#[derive(Debug, Clone)]
pub enum DiscreteStockState {
    Empty { occupied: u32, empty: u32 },
    Normal { occupied: u32, empty: u32 },
    Full { occupied: u32, empty: u32 },
}

impl DiscreteStockState {
    pub fn get_name(&self) -> String {
        match self {
            DiscreteStockState::Empty { .. } => "Empty".to_string(),
            DiscreteStockState::Normal { .. } => "Normal".to_string(),
            DiscreteStockState::Full { .. } => "Full".to_string(),
        }
    }
}

impl StateEq for DiscreteStockState {
    fn is_same_state(&self, other: &Self) -> bool {
        match (self, other) {
            (DiscreteStockState::Empty { .. }, DiscreteStockState::Empty { ..  }) => true,
            (DiscreteStockState::Normal { .. }, DiscreteStockState::Normal { .. }) => true,
            (DiscreteStockState::Full { .. }, DiscreteStockState::Full { .. }) => true,
            _ => false,
        }
    }
}

pub struct DiscreteStock<T> where T: Clone + Default + Send + 'static {
    pub element_name: String,
    pub element_type: String,
    pub resource: ItemDeque<T>,
    pub log_emitter: Output<DiscreteStockLog<T>>,
    pub state_emitter: Output<NotificationMetadata>,
    pub low_capacity: u32,
    pub max_capacity: u32,
    pub prev_state: Option<DiscreteStockState>,
    next_event_id: u64,
}
impl<T: Clone + Default + Send + 'static> Default for DiscreteStock<T> {
    fn default() -> Self {
        DiscreteStock {
            element_name: "DiscreteStock".to_string(),
            element_type: "DiscreteStock".to_string(),
            resource: ItemDeque::default(),
            log_emitter: Output::new(),
            state_emitter: Output::new(),
            low_capacity: 0,
            max_capacity: 1,
            prev_state: None,
            next_event_id: 0,
        }
    }
}

use std::ops::{Deref, DerefMut};

#[derive(Debug, Clone)]

pub struct ItemDeque<T>(VecDeque<T>);
impl<T> Deref for ItemDeque<T>    { type Target = VecDeque<T>; fn deref(&self) -> &Self::Target { &self.0 } }
impl<T> DerefMut for ItemDeque<T> { fn deref_mut(&mut self) -> &mut VecDeque<T> { &mut self.0 } }

impl<TT> VectorArithmetic<Option<TT>, (), u32> for ItemDeque<TT> {
    fn add(&mut self, other: Option<TT>) {
        if let Some(item) = other {
            self.push_back(item);
        }
    }
    fn subtract(&mut self, _: ()) -> Option<TT> {
        self.pop_front()
    }
    fn total(&self) -> u32 {
        self.len() as u32
    }
}

impl<TT: Default> Default for ItemDeque<TT> {
    fn default() -> Self {
        ItemDeque(VecDeque::new())
    }
}

pub trait HasUniqueKey<S> {
    fn get_key(&self) -> S;
}

pub struct ItemMap<S, T: HasUniqueKey<S>>(HashMap<S, T>);
impl<S, T: HasUniqueKey<S>> Deref for ItemMap<S, T> { type Target = HashMap<S, T>; fn deref(&self) -> &Self::Target { &self.0 } }
impl<S, T: HasUniqueKey<S>> DerefMut for ItemMap<S, T> { fn deref_mut(&mut self) -> &mut HashMap<S, T> { &mut self.0 } }

impl<S, T: HasUniqueKey<S>> VectorArithmetic<Option<T>, (), u32> for ItemMap<S, T>
where
    S: Eq + std::hash::Hash,
{
    fn add(&mut self, other: Option<T>) {
        if let Some(item) = other {
            self.insert(item.get_key(), item);
        }
    }
    fn subtract(&mut self, _: ()) -> Option<T> {
        if let Some((_, item)) = self.iter_mut().next() {
            let key = item.get_key();
            let item = self.remove(&key);
            return item;
        }
        None
    }
    fn total(&self) -> u32 {
        self.len() as u32
    }
}

impl<T: Clone + Debug + Default + Send> Stock<ItemDeque<T>, Option<T>, (), Option<T>, u32> for DiscreteStock<T> where Self: Model {
    type StockState = DiscreteStockState;
    fn get_state(&mut self) -> Self::StockState {
        let occupied = self.resource.total();
        let empty = self.max_capacity.saturating_sub(occupied); // If occupied beyond capacity, just say no empty space
        if self.resource.total() <= self.low_capacity {
            DiscreteStockState::Empty { occupied, empty }
        } else if self.resource.total() >= self.max_capacity {
            DiscreteStockState::Full { occupied, empty }
        } else {
            DiscreteStockState::Normal { occupied, empty }
        }
    }
    fn get_previous_state(&mut self) -> &Option<Self::StockState> {
        &self.prev_state
    }
    fn set_previous_state(&mut self) {
        self.prev_state = Some(self.get_state());
    }
    fn get_resource(&self) -> &ItemDeque<T> {
        &self.resource
    }
    fn add_impl<'a>(&'a mut self, payload: &'a (Option<T>, NotificationMetadata), cx: &'a mut Context<Self>) -> impl Future<Output = ()> + 'a {
        async move {
            self.resource.add(payload.0.clone());
        }
    }
    fn remove_impl<'a>(&'a mut self, payload: &'a ((), NotificationMetadata), cx: &'a mut Context<Self>) -> impl Future<Output = Option<T>> + 'a {
        async move {
            self.prev_state = Some(self.get_state());
            self.resource.pop_front()
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
            let log = DiscreteStockLog {
                time: time.to_chrono_date_time(0).unwrap().to_string(),
                event_id: self.next_event_id,
                element_name: self.element_name.clone(),
                element_type: self.element_type.clone(),
                log_type,
                state: self.get_state(),
                resource: self.resource.clone(),
            };
            self.log_emitter.send(log).await;
            self.next_event_id += 1;
        }
    }
}

impl<T: Clone + Default + Send> Model for DiscreteStock<T> {}

impl<T: Clone + Default + Debug + Send> DiscreteStock<T> {
    pub fn new() -> Self {
        DiscreteStock::default()
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
        self.resource = ItemDeque(contents.into_iter().collect());
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

pub struct DiscreteStockLogger<T> where T: Send {
    pub name: String,
    pub buffer: EventQueue<DiscreteStockLog<T>>,
}

impl<T> Logger for DiscreteStockLogger<T> where DiscreteStockLog<T>: Serialize, T: Send + 'static {
    type RecordType = DiscreteStockLog<T>;
    fn get_name(&self) -> &String {
        &self.name
    }
    fn get_buffer(self) -> EventQueue<Self::RecordType> {
        self.buffer
    }
    fn new(name: String) -> Self {
        DiscreteStockLogger {
            name,
            buffer: EventQueue::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DiscreteStockLog<T> {
    pub time: String,
    pub event_id: u64,
    pub element_name: String,
    pub element_type: String,
    pub log_type: String,
    pub state: DiscreteStockState,
    pub resource: ItemDeque<T>,
}

impl Serialize for DiscreteStockLog<String> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer {
        let mut state = serializer.serialize_struct("ResourceStockLog", 6)?;
        state.serialize_field("time", &self.time)?;
        state.serialize_field("event_id", &self.event_id)?;
        state.serialize_field("element_name", &self.element_name)?;
        state.serialize_field("element_type", &self.element_type)?;
        state.serialize_field("log_type", &self.log_type)?;
        state.serialize_field("state", &self.state.get_name())?;
        state.serialize_field("resource", &format!("{:?}", self.resource))?;
        state.end()
    }
}

/**
 * U: Parameter type pushed to downstream stock
 * V: Parameter type requested from upstream stock
 * W: Parameter type received from upstream stock
 */
pub struct DiscreteProcess<U: Clone + Send + 'static, V: Clone + Send + 'static, W: Clone + Send + 'static> {
    pub element_name: String,
    pub element_type: String,
    pub req_upstream: Requestor<(), DiscreteStockState>,
    pub req_downstream: Requestor<(), DiscreteStockState>,
    pub withdraw_upstream: Requestor<(V, NotificationMetadata), W>,
    pub push_downstream: Output<(U, NotificationMetadata)>,
    pub process_state: Option<(Duration, W)>,
    pub process_time_distr: Option<Distribution>,
    pub process_quantity_distr: Option<Distribution>,
    pub log_emitter: Output<DiscreteProcessLog<U>>,
    time_to_next_event: Option<Duration>,
    next_event_id: u64,
    pub previous_check_time: MonotonicTime,
}
impl<U: Clone + Send + 'static, V: Clone + Send + 'static, W: Clone + Send + 'static> Default for DiscreteProcess<U, V, W> {
    fn default() -> Self {
        DiscreteProcess {
            element_name: "DiscreteProcess".to_string(),
            element_type: "DiscreteProcess".to_string(),
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

impl<U: Clone + Send + 'static, V: Clone + Send + 'static, W: Clone + Send + 'static> DiscreteProcess<U, V, W> {
    pub fn new() -> Self {
        DiscreteProcess::default()
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


impl<U: Clone + Send + 'static, V: Clone + Send + 'static, W: Clone + Send + 'static> Model for DiscreteProcess<U, V, W> {}

impl<U: Clone + Debug + Send + 'static> Process<ItemDeque<U>, Option<U>, (), u32> for DiscreteProcess<Option<U>, (), Option<U>>
where
    Self: Model,
    ItemDeque<U>: VectorArithmetic<Option<U>, (), u32>,
{
    type LogDetailsType = DiscreteProcessLogType<Option<U>>;

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
                        self.log(time, DiscreteProcessLogType::ProcessSuccess { resource: resource.clone() }).await;
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
                            Some(DiscreteStockState::Normal { .. } | DiscreteStockState::Full { .. }),
                            Some(DiscreteStockState::Empty { .. } | DiscreteStockState::Normal { .. }),
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
                            self.log(time, DiscreteProcessLogType::ProcessStart { resource: moved.clone() }).await;
                            self.time_to_next_event = Some(Duration::from_secs_f64(process_duration_secs));
                        },
                        (Some(DiscreteStockState::Empty { .. }), _ ) => {
                            self.log(time, DiscreteProcessLogType::ProcessFailure { reason: "Upstream is empty" }).await;
                            self.time_to_next_event = None;
                        },
                        (None, _) => {
                            self.log(time, DiscreteProcessLogType::ProcessFailure { reason: "Upstream is not connected" }).await;
                            self.time_to_next_event = None;
                        },
                        (_, Some(DiscreteStockState::Full { .. })) => {
                            self.log(time, DiscreteProcessLogType::ProcessFailure { reason: "Downstream is full" }).await;
                            self.time_to_next_event = None;
                        },
                        (_, None) => {
                            self.log(time, DiscreteProcessLogType::ProcessFailure { reason: "Downstream is not connected" }).await;
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
                        cx.schedule_event(next_time, <Self as Process<ItemDeque<U>, Option<U>, (), u32>>::update_state, notif_meta.clone()).unwrap();
                    };
                }
            };
        }
    }

    fn log<'a>(&'a mut self, time: MonotonicTime, details: DiscreteProcessLogType<Option<U>>) -> impl Future<Output = ()> + Send {
        async move {
            let log = DiscreteProcessLog {
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
pub enum DiscreteProcessLogType<T> {
    ProcessStart { resource: T },
    ProcessSuccess { resource: T },
    ProcessFailure { reason: &'static str },
}


#[derive(Debug, Clone)]
pub struct DiscreteProcessLog<T> {
    pub time: String,
    pub event_id: u64,
    pub element_name: String,
    pub element_type: String,
    pub event: DiscreteProcessLogType<T>,
}

impl Serialize for DiscreteProcessLog<Option<String>> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer {
        let mut state = serializer.serialize_struct("DiscreteProcessLog", 7)?;
        state.serialize_field("time", &self.time)?;
        state.serialize_field("event_id", &self.event_id)?;
        state.serialize_field("element_name", &self.element_name)?;
        state.serialize_field("element_type", &self.element_type)?;
        let (event_type, item, reason): (String, Option<&str>, Option<&str>) = match &self.event {
            DiscreteProcessLogType::ProcessStart { resource } => ("ProcessStart".into(), resource.as_deref(), None),
            DiscreteProcessLogType::ProcessSuccess { resource } => ("ProcessSuccess".into(), resource.as_deref(), None),
            DiscreteProcessLogType::ProcessFailure { reason } => ("ProcessFailure".into(), None, Some(reason)),
        };
        state.serialize_field("event_type", &event_type)?;
        state.serialize_field("item", &item)?;
        state.serialize_field("reason", &reason)?;
        state.end()
    }
}

pub struct DiscreteProcessLogger<T> where T: Send {
    pub name: String,
    pub buffer: EventQueue<DiscreteProcessLog<T>>,
}

impl<T> Logger for DiscreteProcessLogger<T> where DiscreteProcessLog<T>: Serialize, T: Send + 'static {
    type RecordType = DiscreteProcessLog<T>;
    fn get_name(&self) -> &String {
        &self.name
    }
    fn get_buffer(self) -> EventQueue<Self::RecordType> {
        self.buffer
    }
    fn new(name: String) -> Self {
        DiscreteProcessLogger {
            name,
            buffer: EventQueue::new(),
        }
    }
}

/**
 * Source
 */

pub trait ItemFactory<U> {
    fn create_item(&mut self) -> U;
}

pub struct StringItemFactory {
    pub prefix: String,
    pub next_index: u64,
    pub num_digits: usize,
}

impl Default for StringItemFactory {
    fn default() -> Self {
        StringItemFactory {
            prefix: "Item".to_string(),
            next_index: 0,
            num_digits: 4,
        }
    }
}

impl ItemFactory<String> for StringItemFactory {
    fn create_item(&mut self) -> String {
        let item = format!("{}_{:0>width$}", self.prefix, self.next_index, width = self.num_digits);
        self.next_index += 1;
        String::from(item)
    }
}

impl ItemFactory<Option<String>> for StringItemFactory {
    fn create_item(&mut self) -> Option<String> {
        Some(self.create_item())
    }
}

pub struct DiscreteSource<
    T: Clone + Send + 'static,
    // U: Clone + Send + 'static,
    TF: ItemFactory<T>,
> {
    pub element_name: String,
    pub element_type: String,
    pub req_upstream: Requestor<(), DiscreteStockState>,
    pub req_downstream: Requestor<(), DiscreteStockState>,
    pub push_downstream: Output<(T, NotificationMetadata)>,
    pub process_state: Option<(Duration, T)>,
    pub process_time_distr: Option<Distribution>,
    pub process_quantity_distr: Option<Distribution>,
    pub log_emitter: Output<DiscreteProcessLog<T>>,
    time_to_next_event: Option<Duration>,
    next_event_id: u64,
    pub previous_check_time: MonotonicTime,
    pub item_factory: TF,
}

impl<T: Clone + Send + 'static, TF: ItemFactory<T> + Default> Default for DiscreteSource<T, TF> {
    fn default() -> Self {
        DiscreteSource {
            element_name: "DiscreteSource".to_string(),
            element_type: "DiscreteSource".to_string(),
            req_upstream: Requestor::new(),
            req_downstream: Requestor::new(),
            push_downstream: Output::new(),
            process_state: None,
            process_time_distr: None,
            process_quantity_distr: None,
            log_emitter: Output::new(),
            time_to_next_event: None,
            next_event_id: 0,
            previous_check_time: MonotonicTime::EPOCH,
            item_factory: TF::default(),
        }
    }
}

impl<T: Clone + Send + 'static, TF: ItemFactory<T> + Default> DiscreteSource<T, TF> {
    pub fn new() -> Self {
        DiscreteSource::default()
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


impl<T: Clone + Send + 'static, TF: ItemFactory<T> + Send + 'static> Model for DiscreteSource<T, TF> {}

impl<T: Clone + Debug + Send + 'static, TF: ItemFactory<T> + Send + 'static> Process<ItemDeque<T>, Option<T>, (), u32> for DiscreteSource<T, TF>
where
    Self: Model,
    ItemDeque<T>: VectorArithmetic<Option<T>, (), u32>,
{
    type LogDetailsType = DiscreteProcessLogType<T>;

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
                        self.log(time, DiscreteProcessLogType::ProcessSuccess { resource: resource.clone() }).await;
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
                    let ds_state = self.req_downstream.send(()).await.next();
                    match &ds_state {
                        Some(DiscreteStockState::Empty { .. } | DiscreteStockState::Normal { .. }) => {
                            let process_duration_secs = self.process_time_distr.as_mut().unwrap_or_else(|| {
                                panic!("Process time distribution not set for source {}", self.element_name);
                            }).sample();

                            let next_item = self.item_factory.create_item();

                            self.process_state = Some((Duration::from_secs_f64(process_duration_secs.clone()), next_item.clone()));
                            self.log(time, DiscreteProcessLogType::ProcessStart { resource: next_item.clone() }).await;
                            self.time_to_next_event = Some(Duration::from_secs_f64(process_duration_secs));
                        },
                        Some(DiscreteStockState::Full { .. }) => {
                            self.log(time, DiscreteProcessLogType::ProcessFailure { reason: "Downstream is full" }).await;
                            self.time_to_next_event = None;
                        },
                        None => {
                            self.log(time, DiscreteProcessLogType::ProcessFailure { reason: "Downstream is not connected" }).await;
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
                        cx.schedule_event(next_time, <Self as Process<ItemDeque<T>, Option<T>, (), u32>>::update_state, notif_meta.clone()).unwrap();
                    };
                }
            };
        }
    }

    fn log<'a>(&'a mut self, time: MonotonicTime, details: Self::LogDetailsType) -> impl Future<Output = ()> + Send {
        async move {
            let log = DiscreteProcessLog {
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