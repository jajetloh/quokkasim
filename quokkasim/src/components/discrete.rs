use serde::ser::SerializeStruct;
use serde::Serialize;

use crate::prelude::*;
use std::collections::{VecDeque, HashMap};
use std::time::Duration;
use std::fmt::Debug;
use std::ops::{Deref, DerefMut};

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


#[derive(Debug, Clone)]
pub struct ItemDeque<T>(VecDeque<T>);
impl<T> Deref for ItemDeque<T>    { type Target = VecDeque<T>; fn deref(&self) -> &Self::Target { &self.0 } }
impl<T> DerefMut for ItemDeque<T> { fn deref_mut(&mut self) -> &mut VecDeque<T> { &mut self.0 } }

impl<T: Default> Default for ItemDeque<T> {
    fn default() -> Self {
        ItemDeque(VecDeque::new())
    }
}

impl<T> ResourceAdd<T> for ItemDeque<T> {
    fn add(&mut self, item: T) {
        self.push_back(item);
    }
}

impl<T> ResourceRemove<(), Option<T>> for ItemDeque<T> {
    fn remove(&mut self, _: ()) -> Option<T> {
        self.pop_front()
    }
}

impl<T> ResourceTotal<u32> for ItemDeque<T> {
    fn total(&self) -> u32 {
        self.len() as u32
    }
}

pub trait HasUniqueKey<S> {
    fn get_key(&self) -> S;
}

pub struct ItemMap<S, T: HasUniqueKey<S>>(HashMap<S, T>);
impl<S, T: HasUniqueKey<S>> Deref for ItemMap<S, T> { type Target = HashMap<S, T>; fn deref(&self) -> &Self::Target { &self.0 } }
impl<S, T: HasUniqueKey<S>> DerefMut for ItemMap<S, T> { fn deref_mut(&mut self) -> &mut HashMap<S, T> { &mut self.0 } }

impl<T> ResourceAdd<T> for ItemMap<String, T>
where
    T: HasUniqueKey<String>,
{
    fn add(&mut self, item: T) {
        self.insert(item.get_key(), item);
    }
}

impl<T> ResourceRemove<String, Option<T>> for ItemMap<String, T>
where
    T: HasUniqueKey<String>,
{
    fn remove(&mut self, key: String) -> Option<T> {
        HashMap::<String, T>::remove(self, &key)
    }
}

impl<T> ResourceTotal<u32> for ItemMap<String, T>
where
    T: HasUniqueKey<String>,
{
    fn total(&self) -> u32 {
        self.len() as u32
    }
}

impl<T: Clone + Default + Send> Stock<ItemDeque<T>, T, (), Option<T>> for DiscreteStock<T> {
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
    fn add_impl(&mut self, payload: &(T, NotificationMetadata), cx: &mut Context<Self>) -> impl Future<Output = ()> {
        async move {
            self.resource.add(payload.0.clone());
        }
    }
    fn remove_impl(&mut self, payload: &((), NotificationMetadata), cx: &mut Context<Self>) -> impl Future<Output = Option<T>> {
        async move {
            self.prev_state = Some(self.get_state());
            self.resource.pop_front()
        }
    }

    fn emit_change(&mut self, payload: NotificationMetadata, cx: &mut nexosim::model::Context<Self>) -> impl Future<Output = ()> {
        async move {
            self.state_emitter.send(payload).await;
            self.log(cx.time(), "Emit Change".to_string()).await;
        }
    }

    fn log(&mut self, time: MonotonicTime, log_type: String) -> impl Future<Output = ()> {
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

impl<T: Clone + Default + Send> DiscreteStock<T> {
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

impl<T> Logger for DiscreteStockLogger<T> where T: Serialize + Send + 'static {
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

impl<T: Serialize> Serialize for DiscreteStockLog<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer {
        let mut state = serializer.serialize_struct("DiscreteStockLog", 6)?;
        state.serialize_field("time", &self.time)?;
        state.serialize_field("event_id", &self.event_id)?;
        state.serialize_field("element_name", &self.element_name)?;
        state.serialize_field("element_type", &self.element_type)?;
        state.serialize_field("log_type", &self.log_type)?;
        state.serialize_field("state", &self.state.get_name())?;
        let resource_str: String = serde_json::to_string(&self.resource.0)
            .map_err(|e| serde::ser::Error::custom(format!("Failed to serialize resource: {}", e)))?;
        state.serialize_field("resource", &resource_str)?;
        state.end()
    }
}

pub struct DiscreteProcess<
    ReceiveParameterType: Clone + Send + 'static,
    ReceiveType: Clone + Send + 'static,
    InternalResourceType: Clone + Send + 'static,
    SendType: Clone + Send + 'static,
> {
    pub element_name: String,
    pub element_type: String,
    pub req_upstream: Requestor<(), DiscreteStockState>,
    pub req_downstream: Requestor<(), DiscreteStockState>,
    pub withdraw_upstream: Requestor<(ReceiveParameterType, NotificationMetadata), ReceiveType>,
    pub push_downstream: Output<(SendType, NotificationMetadata)>,
    pub process_state: Option<(Duration, InternalResourceType)>,
    pub process_time_distr: Option<Distribution>,
    pub process_quantity_distr: Option<Distribution>,
    pub log_emitter: Output<DiscreteProcessLog<InternalResourceType>>,
    time_to_next_event: Option<Duration>,
    next_event_id: u64,
    pub previous_check_time: MonotonicTime,
}
impl<U: Clone + Send + 'static, V: Clone + Send + 'static, W: Clone + Send + 'static, X: Clone + Send + 'static> Default for DiscreteProcess<U, V, W, X> {
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

impl<
    ReceiveParameterType: Clone + Send + 'static,
    ReceiveType: Clone + Send + 'static,
    InternalResourceType: Clone + Send + 'static,
    SendType: Clone + Send + 'static,
> DiscreteProcess<ReceiveParameterType, ReceiveType, InternalResourceType, SendType> {
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


impl<
    ReceiveParameterType: Clone + Send + 'static,
    ReceiveType: Clone + Send + 'static,
    InternalResourceType: Clone + Send + 'static,
    SendType: Clone + Send + 'static,
> Model for DiscreteProcess<ReceiveParameterType, ReceiveType, InternalResourceType, SendType> where Self: Process {
    fn init(mut self, ctx: &mut Context<Self>) -> impl Future<Output = InitializedModel<Self>> + Send {
        async move {
            let notif_meta = NotificationMetadata {
                time: ctx.time(),
                element_from: self.element_name.clone(),
                message: "Initialisation".into(),
            };
            self.update_state(notif_meta, ctx).await;
            self.into()
        }
    } 
}

impl<T: Clone + Send + 'static> Process for DiscreteProcess<(), Option<T>, T, T> {
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

    fn update_state_impl(&mut self, notif_meta: &NotificationMetadata, cx: &mut Context<Self>) -> impl Future<Output = ()> {
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
                            let received = self.withdraw_upstream.send(((), NotificationMetadata {
                                time,
                                element_from: self.element_name.clone(),
                                message: "Withdraw request".into(),
                            })).await.next().unwrap();
                            match received {
                                Some(received_resource) => {
                                    let process_duration_secs = self.process_time_distr.as_mut().unwrap_or_else(|| {
                                        panic!("Process time distribution not set for process {}", self.element_name);
                                    }).sample();
                                    self.process_state = Some((Duration::from_secs_f64(process_duration_secs.clone()), received_resource.clone()));
                                    self.log(time, DiscreteProcessLogType::ProcessStart { resource: received_resource }).await;
                                    self.time_to_next_event = Some(Duration::from_secs_f64(process_duration_secs));
                                },
                                None => {
                                    self.log(time, DiscreteProcessLogType::ProcessFailure { reason: "Upstream did not provide resource" }).await;
                                    self.time_to_next_event = None;
                                }
                            }
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

    fn post_update_state(&mut self, notif_meta: &NotificationMetadata, cx: &mut Context<Self>) -> impl Future<Output = ()> {
        async move {
            self.set_previous_check_time(cx.time());
            match self.time_to_next_event {
                None => {},
                Some(time_until_next) => {
                    if time_until_next.is_zero() {
                        panic!("Time until next event is zero!");
                    } else {
                        let next_time = cx.time() + time_until_next;
                        cx.schedule_event(next_time, <Self as Process>::update_state, notif_meta.clone()).unwrap();
                    };
                }
            };
        }
    }

    fn log(&mut self, time: MonotonicTime, details: DiscreteProcessLogType<T>) -> impl Future<Output = ()> {
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

impl<T: Serialize> Serialize for DiscreteProcessLog<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer {
        let mut state = serializer.serialize_struct("DiscreteProcessLog", 7)?;
        state.serialize_field("time", &self.time)?;
        state.serialize_field("event_id", &self.event_id)?;
        state.serialize_field("element_name", &self.element_name)?;
        state.serialize_field("element_type", &self.element_type)?;
        let (event_type, item, reason): (String, Option<String>, Option<&str>) = match &self.event {
            DiscreteProcessLogType::ProcessStart { resource } => ("ProcessStart".into(), Some(serde_json::to_string(resource).unwrap()), None),
            DiscreteProcessLogType::ProcessSuccess { resource } => ("ProcessSuccess".into(), Some(serde_json::to_string(resource).unwrap()), None),
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

impl<T> Logger for DiscreteProcessLogger<T> where T: Serialize, T: Send + 'static {
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

pub struct DiscreteSource<
    InternalResourceType: Clone + Send + 'static,
    SendType: Clone + Send + 'static,
    FactoryType: ItemFactory<InternalResourceType>,
> {
    pub element_name: String,
    pub element_type: String,
    pub req_upstream: Requestor<(), DiscreteStockState>,
    pub req_downstream: Requestor<(), DiscreteStockState>,
    pub push_downstream: Output<(SendType, NotificationMetadata)>,
    pub process_state: Option<(Duration, InternalResourceType)>,
    pub process_time_distr: Option<Distribution>,
    pub process_quantity_distr: Option<Distribution>,
    pub log_emitter: Output<DiscreteProcessLog<InternalResourceType>>,
    time_to_next_event: Option<Duration>,
    next_event_id: u64,
    pub previous_check_time: MonotonicTime,
    pub item_factory: FactoryType,
}

impl<
    InternalResourceType: Clone + Send + 'static,
    SendType: Clone + Send + 'static,
    FactoryType: ItemFactory<InternalResourceType> + Default,
> Default for DiscreteSource<InternalResourceType, SendType, FactoryType> {
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
            item_factory: FactoryType::default(),
        }
    }
}

impl<
    InternalResourceType: Clone + Send + 'static,
    SendType: Clone + Send + 'static,
    FactoryType: ItemFactory<InternalResourceType> + Default,
> DiscreteSource<InternalResourceType, SendType, FactoryType> {
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
    pub fn with_item_factory(mut self, item_factory: FactoryType) -> Self {
        self.item_factory = item_factory;
        self
    }
}


impl<
    InternalResourceType: Clone + Send + 'static,
    SendType: Clone + Send + 'static,
    FactoryType: ItemFactory<InternalResourceType> + Send + 'static,
> Model for DiscreteSource<InternalResourceType, SendType, FactoryType> where Self: Process {
    fn init(mut self, ctx: &mut Context<Self>) -> impl Future<Output = InitializedModel<Self>> {
        async move {
            let notif_meta = NotificationMetadata {
                time: ctx.time(),
                element_from: self.element_name.clone(),
                message: "Initialisation".into(),
            };
            self.update_state(notif_meta, ctx).await;
            self.into()
        }
    }
}

impl<
    T: Clone + Send + 'static,
    FactoryType: ItemFactory<T> + Send + 'static,
> Process for DiscreteSource<T, T, FactoryType> {
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

    fn update_state_impl(&mut self, notif_meta: &NotificationMetadata, cx: &mut Context<Self>) -> impl Future<Output = ()> {
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

    fn post_update_state(&mut self, notif_meta: &NotificationMetadata, cx: &mut Context<Self>) -> impl Future<Output = ()> {
        async move {
            self.set_previous_check_time(cx.time());
            match self.time_to_next_event {
                None => {},
                Some(time_until_next) => {
                    if time_until_next.is_zero() {
                        panic!("Time until next event is zero!");
                    } else {
                        let next_time = cx.time() + time_until_next;
                        cx.schedule_event(next_time, <Self as Process>::update_state, notif_meta.clone()).unwrap();
                    };
                }
            };
        }
    }

    fn log(&mut self, time: MonotonicTime, details: Self::LogDetailsType) -> impl Future<Output = ()> {
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

pub struct DiscreteSink<
    RequestParameterType: Clone + Send + 'static,
    RequestType: Clone + Send + 'static,
    InternalResourceType: Clone + Send + 'static,
> {
    pub element_name: String,
    pub element_type: String,
    pub req_upstream: Requestor<(), DiscreteStockState>,
    pub withdraw_upstream: Requestor<(RequestParameterType, NotificationMetadata), RequestType>,
    pub process_state: Option<(Duration, InternalResourceType)>,
    pub process_time_distr: Option<Distribution>,
    pub process_quantity_distr: Option<Distribution>,
    pub log_emitter: Output<DiscreteProcessLog<InternalResourceType>>,
    time_to_next_event: Option<Duration>,
    next_event_id: u64,
    pub previous_check_time: MonotonicTime,
}

impl<
    RequestParameterType: Clone + Send + 'static,
    RequestType: Clone + Send + 'static,
    InternalResourceType: Clone + Send + 'static
> Default for DiscreteSink<RequestParameterType, RequestType, InternalResourceType> {
    fn default() -> Self {
        DiscreteSink {
            element_name: "DiscreteSink".to_string(),
            element_type: "DiscreteSink".to_string(),
            req_upstream: Requestor::new(),
            withdraw_upstream: Requestor::new(),
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

impl<
    RequestParameterType: Clone + Send + 'static,
    RequestType: Clone + Send + 'static,
    InternalResourceType: Clone + Send + 'static
> DiscreteSink<RequestParameterType, RequestType, InternalResourceType> {
    pub fn new() -> Self {
        DiscreteSink::default()
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

impl<
    RequestParameterType: Clone + Send + 'static,
    RequestType: Clone + Send + 'static,
    InternalResourceType: Clone + Send + 'static
> Model for DiscreteSink<RequestParameterType, RequestType, InternalResourceType> where Self: Process {
    fn init(mut self, ctx: &mut Context<Self>) -> impl Future<Output = InitializedModel<Self>> {
        async move {
            let notif_meta = NotificationMetadata {
                time: ctx.time(),
                element_from: self.element_name.clone(),
                message: "Initialisation".into(),
            };
            self.update_state(notif_meta, ctx).await;
            self.into()
        }
    }
}

impl<T: Clone + Send + 'static> Process for DiscreteSink<(), Option<T>, T> {
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

    fn update_state_impl(&mut self, notif_meta: &NotificationMetadata, cx: &mut Context<Self>) -> impl Future<Output = ()> {
        async move {
            let time = cx.time();

            match self.process_state.take() {
                Some((mut process_time_left, resource)) => {
                    let duration_since_prev_check = cx.time().duration_since(self.previous_check_time);
                    process_time_left = process_time_left.saturating_sub(duration_since_prev_check);
                    if process_time_left.is_zero() {
                        self.log(time, DiscreteProcessLogType::ProcessSuccess { resource: resource.clone() }).await;
                    } else {
                        self.process_state = Some((process_time_left, resource));
                    }
                }
                None => {}
            }
            match self.process_state {
                None => {
                    let us_state = self.req_upstream.send(()).await.next();
                    match &us_state {
                        Some(DiscreteStockState::Normal { .. } | DiscreteStockState::Full { .. }) => {
                            let moved = self.withdraw_upstream.send(((), NotificationMetadata {
                                time,
                                element_from: self.element_name.clone(),
                                message: "Withdraw request".into(),
                            })).await.next().unwrap();
                            match moved {
                                Some(moved) => {
                                    let process_duration_secs = self.process_time_distr.as_mut().unwrap_or_else(|| {
                                        panic!("Process time distribution not set for sink {}", self.element_name);
                                    }).sample();
                                    self.process_state = Some((Duration::from_secs_f64(process_duration_secs.clone()), moved.clone()));
                                    self.log(time, DiscreteProcessLogType::ProcessStart { resource: moved }).await;
                                    self.time_to_next_event = Some(Duration::from_secs_f64(process_duration_secs));
                                },
                                None => {
                                    self.log(time, DiscreteProcessLogType::ProcessFailure { reason: "Upstream did not provide resource" }).await;
                                    self.time_to_next_event = None;
                                    return;
                                }
                            }
                        },
                        Some(DiscreteStockState::Empty { .. }) => {
                            self.log(time, DiscreteProcessLogType::ProcessFailure { reason: "Upstream is empty" }).await;
                            self.time_to_next_event = None;
                        }
                        None => {
                            self.log(time, DiscreteProcessLogType::ProcessFailure { reason: "Upstream is not connected" }).await;
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

    fn post_update_state(&mut self, notif_meta: &NotificationMetadata, cx: &mut Context<Self>) -> impl Future<Output = ()> {
        async move {
            self.set_previous_check_time(cx.time());
            match self.time_to_next_event {
                None => {},
                Some(time_until_next) => {
                    if time_until_next.is_zero() {
                        panic!("Time until next event is zero!");
                    } else {
                        let next_time = cx.time() + time_until_next;
                        cx.schedule_event(next_time, <Self as Process>::update_state, notif_meta.clone()).unwrap();
                    };
                }
            };
        }
    }

    fn log(&mut self, time: MonotonicTime, details: Self::LogDetailsType) -> impl Future<Output = ()> {
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

pub struct DiscreteParallelProcess<
    ReceiveParameterType: Clone + Send + 'static,
    ReceiveType: Clone + Send + 'static,
    InternalResourceType: Clone + Send + 'static,
    SendType: Clone + Send + 'static,
> {
    pub element_name: String,
    pub element_type: String,
    pub req_upstream: Requestor<(), DiscreteStockState>,
    pub req_downstream: Requestor<(), DiscreteStockState>,
    pub withdraw_upstream: Requestor<(ReceiveParameterType, NotificationMetadata), ReceiveType>,
    pub push_downstream: Output<(SendType, NotificationMetadata)>,
    pub processes_in_progress: Vec<(Duration, InternalResourceType)>,
    pub processes_complete: VecDeque<SendType>,
    pub process_time_distr: Option<Distribution>,
    pub process_quantity_distr: Option<Distribution>,
    pub log_emitter: Output<DiscreteProcessLog<SendType>>,
    time_to_next_event: Option<Duration>,
    next_event_id: u64,
    pub previous_check_time: MonotonicTime,
}

impl<
    ReceiveParameterType: Clone + Send + 'static,
    ReceiveType: Clone + Send + 'static,
    InternalResourceType: Clone + Send + 'static,
    SendType: Clone + Send + 'static
> Default for DiscreteParallelProcess<ReceiveParameterType, ReceiveType, InternalResourceType, SendType> {
    fn default() -> Self {
        DiscreteParallelProcess {
            element_name: "DiscreteParallelProcess".to_string(),
            element_type: "DiscreteParallelProcess".to_string(),
            req_upstream: Requestor::new(),
            req_downstream: Requestor::new(),
            withdraw_upstream: Requestor::new(),
            push_downstream: Output::new(),
            processes_in_progress: Vec::new(),
            processes_complete: VecDeque::new(),
            process_time_distr: None,
            process_quantity_distr: None,
            log_emitter: Output::new(),
            time_to_next_event: None,
            next_event_id: 0,
            previous_check_time: MonotonicTime::EPOCH,
        }
    }
}

impl<
    ReceiveParameterType: Clone + Send + 'static,
    ReceiveType: Clone + Send + 'static,
    InternalResourceType: Clone + Send + 'static,
    SendType: Clone + Send + 'static
> DiscreteParallelProcess<ReceiveParameterType, ReceiveType, InternalResourceType, SendType> {
    pub fn new() -> Self {
        DiscreteParallelProcess::default()
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

impl<
    ReceiveParameterType: Clone + Send + 'static,
    ReceiveType: Clone + Send + 'static,
    InternalResourceType: Clone + Send + 'static,
    SendType: Clone + Send + 'static
> Model for DiscreteParallelProcess<ReceiveParameterType, ReceiveType, InternalResourceType, SendType> where Self: Process {
    fn init(mut self, ctx: &mut Context<Self>) -> impl Future<Output = InitializedModel<Self>> {
        async move {
            let notif_meta = NotificationMetadata {
                time: ctx.time(),
                element_from: self.element_name.clone(),
                message: "Initialisation".into(),
            };
            self.update_state(notif_meta, ctx).await;
            self.into()
        }
    }
}

impl<U: Clone + Send + 'static> Process for DiscreteParallelProcess<(), Option<U>, U, U> {
    type LogDetailsType = DiscreteProcessLogType<U>;

    fn get_time_to_next_event(&mut self) -> &Option<Duration> { 
        &self.time_to_next_event
    }

    fn set_time_to_next_event(&mut self, time: Option<Duration>) {
        self.time_to_next_event = time;
    }

    fn set_previous_check_time(&mut self, time: MonotonicTime) {
        self.previous_check_time = time;
    }

    fn update_state_impl(&mut self, notif_meta: &NotificationMetadata, cx: &mut Context<Self>) -> impl Future<Output = ()> {
        async move {
            // First resolve any completed processes

            let time = cx.time();
            let duration_since_prev_check = cx.time().duration_since(self.previous_check_time);
            self.processes_in_progress.retain_mut(|(process_time_left, item)| {
                *process_time_left = process_time_left.saturating_sub(duration_since_prev_check);
                if process_time_left.is_zero() {
                    self.processes_complete.push_back(item.clone());
                    false
                } else {
                    true
                }
            });
            while let Some(item) = self.processes_complete.pop_front() {
                let ds_state = self.req_downstream.send(()).await.next();
                match &ds_state {
                    Some(DiscreteStockState::Empty { .. } | DiscreteStockState::Normal { .. }) => {
                        self.push_downstream.send((item.clone(), NotificationMetadata {
                            time,
                            element_from: self.element_name.clone(),
                            message: "ProcessComplete".into(),
                        })).await;
                        self.log(time, DiscreteProcessLogType::ProcessSuccess { resource: item.clone() }).await;
                    },
                    Some(DiscreteStockState::Full { .. }) => {
                        self.log(time, DiscreteProcessLogType::ProcessFailure { reason: "Downstream is full" }).await;
                        break;
                    },
                    None => {
                        self.log(time, DiscreteProcessLogType::ProcessFailure { reason: "Downstream is not connected" }).await;
                        break;
                    }
                }
            }

            // Then check for any processes to start

            loop {
                let us_state = self.req_upstream.send(()).await.next();
                match &us_state {
                    Some(DiscreteStockState::Empty { .. } | DiscreteStockState::Normal { .. }) => {
                        let item = self.withdraw_upstream.send(((), NotificationMetadata {
                            time,
                            element_from: self.element_name.clone(),
                            message: "Withdraw request".into(),
                        })).await.next().unwrap();
                        if let Some(item) = item {
                            let process_duration = Duration::from_secs_f64(self.process_time_distr.as_mut().unwrap_or_else(|| {
                                panic!("Process time distribution not set for process {}", self.element_name);
                            }).sample());

                            self.processes_in_progress.push((process_duration, item.clone()));
                            self.log(time, DiscreteProcessLogType::ProcessStart { resource: item }).await;

                        } else {
                            break;
                        }
                    },
                    Some(DiscreteStockState::Full { .. }) => {
                        self.log(time, DiscreteProcessLogType::ProcessFailure { reason: "Upstream is full" }).await;
                        break;
                    },
                    None => {
                        self.log(time, DiscreteProcessLogType::ProcessFailure { reason: "Upstream is not connected" }).await;
                        break;
                    }
                }
            }
            self.time_to_next_event = if self.processes_in_progress.is_empty() {
                None
            } else {
                // Find the minimum time to next event
                let min_time = self.processes_in_progress.iter().map(|(time, _)| *time).min().unwrap();
                Some(min_time)
            };
        }
    }

    fn post_update_state(&mut self, notif_meta: &NotificationMetadata, cx: &mut Context<Self>) -> impl Future<Output = ()> {
        async move {
            self.set_previous_check_time(cx.time());
            match self.time_to_next_event {
                None => {},
                Some(time_until_next) => {
                    if time_until_next.is_zero() {
                        panic!("Time until next event is zero!");
                    } else {
                        let next_time = cx.time() + time_until_next;
                        cx.schedule_event(next_time, <Self as Process>::update_state, notif_meta.clone()).unwrap();
                    };
                }
            };
        }
    }

    fn log(&mut self, time: MonotonicTime, details: Self::LogDetailsType) -> impl Future<Output = ()> {
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
