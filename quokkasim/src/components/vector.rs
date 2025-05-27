use futures::{future::join_all};
use nexosim::{model::Model, ports::{EventQueue, Output, Requestor}};
use serde::{ser::SerializeStruct, Serialize};
use tai_time::MonotonicTime;
use std::{fmt::Debug, time::Duration};

use crate::{core::{StateEq, Process, Stock}, prelude::{Vector3, VectorArithmetic}};
use crate::core::Logger;
use crate::common::{Distribution, NotificationMetadata};

/**
 * Stock
 */

#[derive(Debug, Clone)]
pub enum VectorStockState {
    Empty { occupied: f64, empty: f64 },
    Normal { occupied: f64, empty: f64 },
    Full { occupied: f64, empty: f64 },
}

impl VectorStockState {
    pub fn get_name(&self) -> String {
        match self {
            VectorStockState::Empty { .. } => "Empty".to_string(),
            VectorStockState::Normal { .. } => "Normal".to_string(),
            VectorStockState::Full { .. } => "Full".to_string(),
        }
    }
}

impl StateEq for VectorStockState {
    fn is_same_state(&self, other: &Self) -> bool {
        match (self, other) {
            (VectorStockState::Empty { .. }, VectorStockState::Empty { .. }) => true,
            (VectorStockState::Normal { .. }, VectorStockState::Normal { .. }) => true,
            (VectorStockState::Full { .. }, VectorStockState::Full { .. }) => true,
            _ => false,
        }
    }
}

pub struct VectorStock<T: VectorArithmetic<T, f64, f64> + Clone + Debug + Send + 'static> {
    pub element_name: String,
    pub element_type: String,
    pub vector: T,
    pub log_emitter: Output<VectorStockLog<T>>,
    pub state_emitter: Output<NotificationMetadata>,
    pub low_capacity: f64,
    pub max_capacity: f64,
    pub prev_state: Option<VectorStockState>,
    next_event_id: u64,
}
impl<T: VectorArithmetic<T, f64, f64> + Clone + Debug + Default + Send> Default for VectorStock<T> {
    fn default() -> Self {
        VectorStock {
            element_name: String::new(),
            element_type: String::new(),
            vector: Default::default(),
            low_capacity: 0.0,
            max_capacity: 0.0,
            log_emitter: Output::default(),
            state_emitter: Output::default(),
            prev_state: None,
            next_event_id: 0,
        }
    }
}

impl<T: VectorArithmetic<T, f64, f64> + Clone + Debug + Send> Stock<T, T, f64, T, f64> for VectorStock<T> where Self: Model {

    type StockState = VectorStockState;

    fn get_state(&mut self) -> Self::StockState {
        let occupied = self.vector.total();
        let empty = self.max_capacity - occupied;
        if empty <= 0.0 {
            VectorStockState::Full { occupied, empty }
        } else if occupied < self.low_capacity {
            VectorStockState::Empty { occupied, empty }
        } else {
            VectorStockState::Normal { occupied, empty }
        }
    }

    fn get_previous_state(&mut self) -> &Option<Self::StockState> {
        &self.prev_state
    }
    fn set_previous_state(&mut self) {
        self.prev_state = Some(self.get_state());
    }
    fn get_resource(&self) -> &T {
        &self.vector
    }

    fn add_impl<'a>(
        &'a mut self,
        payload: &'a (T, NotificationMetadata),
        cx: &'a mut ::nexosim::model::Context<Self>
    ) -> impl Future<Output=()> + 'a {
        async move {
            self.prev_state = Some(self.get_state().clone());
            self.vector.add(payload.0.clone());
        }
    }

    fn remove_impl<'a>(
        &'a mut self,
        data: &'a (f64, NotificationMetadata),
        cx: &'a mut ::nexosim::model::Context<Self>
    ) -> impl Future<Output=T> + 'a {
        async move {
            self.prev_state = Some(self.get_state());
            self.vector.subtract(data.0.clone())
        }
    }

    fn emit_change(&mut self, payload: NotificationMetadata, cx: &mut nexosim::model::Context<Self>) -> impl Future<Output=()> + Send {
        async move {
            self.state_emitter.send(payload).await;
            self.log(cx.time(), "Emit Change".to_string()).await;
        }
    }

    fn log(&mut self, time: MonotonicTime, log_type: String) -> impl Future<Output=()> + Send {
        async move {
            let log = VectorStockLog {
                time: time.to_chrono_date_time(0).unwrap().to_string(),
                event_id: self.next_event_id,
                element_name: self.element_name.clone(),
                element_type: self.element_type.clone(),
                log_type,
                state: self.get_state(),
                vector: self.vector.clone(),
            };
            self.next_event_id += 1;
            self.log_emitter.send(log).await;
        }
    }
}

impl<T: VectorArithmetic<T, f64, f64> + Clone + Debug + Send> VectorStock<T> where Self: Model {
    pub fn get_state(&mut self) -> VectorStockState {
        let occupied = self.vector.total();
        let empty = self.max_capacity - occupied;
        if empty <= 0.0 {
            VectorStockState::Full { occupied, empty }
        } else if occupied < self.low_capacity {
            VectorStockState::Empty { occupied, empty }
        } else {
            VectorStockState::Normal { occupied, empty }
        }
    }

    pub fn with_name(self, name: String) -> Self {
        VectorStock {
            element_name: name,
            ..self
        }
    }

    pub fn with_type(self, element_type: String) -> Self {
        VectorStock {
            element_type,
            ..self
        }
    }

    pub fn new() -> Self where T: Default {
        Self::default()
    }

    pub fn with_low_capacity(self, low_capacity: f64) -> Self {
        VectorStock {
            low_capacity,
            ..self
        }
    }

    pub fn with_low_capacity_inplace(&mut self, low_capacity: f64) {
        self.low_capacity = low_capacity;
    }

    pub fn with_max_capacity(self, max_capacity: f64) -> Self {
        VectorStock {
            max_capacity,
            ..self
        }
    }

    pub fn with_initial_vector(self, vector: T) -> Self {
        VectorStock {
            vector,
            ..self
        }
    }
}

impl<T: Debug + Clone + Send + VectorArithmetic<T, f64, f64>> Model for VectorStock<T> {}

pub struct VectorStockLogger<T> where T: Send {
    pub name: String,
    pub buffer: EventQueue<VectorStockLog<T>>,
}

#[derive(Debug, Clone)]
pub struct VectorStockLog<T> {
    pub time: String,
    pub event_id: u64,
    pub element_name: String,
    pub element_type: String,
    pub log_type: String,
    pub state: VectorStockState,
    pub vector: T,
}

impl<T: Serialize> Serialize for VectorStockLog<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("VectorStockLog", 7)?;
        state.serialize_field("time", &self.time)?;
        state.serialize_field("event_id", &self.event_id)?;
        state.serialize_field("element_name", &self.element_name)?;
        state.serialize_field("element_type", &self.element_type)?;
        state.serialize_field("log_type", &self.log_type)?;
        state.serialize_field("state", &self.state.get_name())?;
        let vector_json = serde_json::to_string(&self.vector).map_err(serde::ser::Error::custom)?;
        state.serialize_field("vector", &vector_json)?;
        state.end()
    }
}

impl<T: Serialize + Send + 'static> Logger for VectorStockLogger<T> {
    type RecordType = VectorStockLog<T>;
    fn get_name(&self) -> &String {
        &self.name
    }
    fn get_buffer(self) -> EventQueue<Self::RecordType> {
        self.buffer
    }
    fn new(name: String) -> Self {
        VectorStockLogger {
            name,
            buffer: EventQueue::new(),
        }
    }
}

/**
 * Process
 */

 /**
  * T: Resource type of upstream stock
  * U: Message type for pushing to downstream stock
  * V: Message type for withdrawing from upstream stock
  *
  * M: Number of upstream stocks
  * N: Number of downstream stocks
  */
pub struct VectorProcess<
    T: VectorArithmetic<T, f64, f64> + Clone + Debug + Send + 'static,
    U: Clone + Send + 'static,
    V: Clone + Send + 'static,
> {
    pub element_name: String,
    pub element_type: String,
    pub req_upstream: Requestor<(), VectorStockState>,
    pub req_downstream: Requestor<(), VectorStockState>,
    pub withdraw_upstream: Requestor<(V, NotificationMetadata), T>,
    pub push_downstream: Output<(U, NotificationMetadata)>,
    pub process_state: Option<(Duration, T)>,
    pub process_quantity_distr: Distribution,
    pub process_time_distr: Distribution,
    pub time_to_next_event: Option<Duration>,
    next_event_id: u64,
    pub log_emitter: Output<VectorProcessLog<T>>,
    pub previous_check_time: MonotonicTime,
}
impl<
    T: VectorArithmetic<T, f64, f64> + Clone + Debug + Default + Send,
    U: Clone + Send,
    V: Clone + Send,
> Default for VectorProcess<T, U, V> {
    fn default() -> Self {
        VectorProcess {
            element_name: String::new(),
            element_type: String::new(),
            process_quantity_distr: Distribution::default(),
            process_time_distr: Distribution::default(),

            req_upstream: Requestor::default(),
            req_downstream: Requestor::default(),
            withdraw_upstream: Requestor::default(),
            push_downstream: Output::default(),
            log_emitter: Output::default(),
            
            process_state: None,
            time_to_next_event: None,
            next_event_id: 0,
            previous_check_time: MonotonicTime::EPOCH,
        }
    }
}

impl<T: VectorArithmetic<T, f64, f64> + Send + 'static + Clone + Debug, U: Clone + Send, V: Clone + Send> Model for VectorProcess<T, U, V> {}

impl<T: VectorArithmetic<T, f64, f64> + Send + 'static + Clone + Debug> Process<T, T, f64, f64> for VectorProcess<T, T, f64> where Self: Model {

    type LogDetailsType = VectorProcessLogType<T>;

    fn get_time_to_next_event(&mut self) -> &Option<Duration> {
        &self.time_to_next_event 
    }
    fn set_time_to_next_event(&mut self, time: Option<Duration>) {
        self.time_to_next_event = time;
    }
    fn set_previous_check_time(&mut self, time: MonotonicTime) {
        self.previous_check_time = time;
    }
    fn update_state_impl<'a> (&'a mut self, notif_meta: &NotificationMetadata, cx: &'a mut nexosim::model::Context<Self>) -> impl Future<Output = ()> + 'a where Self: Model {
        async move {
            let time = cx.time();

            match self.process_state.take() {
                Some((mut process_time_left, resource)) => {
                    let duration_since_prev_check = cx.time().duration_since(self.previous_check_time);
                    process_time_left = process_time_left.saturating_sub(duration_since_prev_check);
                    if process_time_left.is_zero() {
                        self.log(time, VectorProcessLogType::ProcessSuccess { quantity: resource.total(), vector: resource.clone() }).await;
                        self.push_downstream.send((resource.clone(), NotificationMetadata {
                            time,
                            element_from: self.element_name.clone(),
                            message: format!("Pushing quantity {:?}", resource),
                        })).await;
                    } else {
                        self.process_state = Some((process_time_left, resource));
                    }
                },
                None => {}
            }
            match self.process_state {
                None => {
                    let us_state = self.req_upstream.send(()).await.next();
                    let ds_state = self.req_downstream.send(()).await.next();
                    match (&us_state, &ds_state) {
                        (
                            Some(VectorStockState::Normal {..}) | Some(VectorStockState::Full {..}),
                            Some(VectorStockState::Empty {..}) | Some(VectorStockState::Normal {..}),
                        ) => {
                            let process_quantity = self.process_quantity_distr.sample();
                            let moved = self.withdraw_upstream.send((process_quantity, NotificationMetadata {
                                time,
                                element_from: self.element_name.clone(),
                                message: format!("Withdrawing quantity {:?}", process_quantity),
                            })).await.next().unwrap();
                            let process_duration_secs = self.process_time_distr.sample();
                            self.process_state = Some((Duration::from_secs_f64(process_duration_secs), moved.clone()));
                            self.log(time, VectorProcessLogType::ProcessStart { quantity: process_quantity, vector: moved }).await;
                            self.time_to_next_event = Some(Duration::from_secs_f64(process_duration_secs));
                        },
                        (Some(VectorStockState::Empty {..} ), _) => {
                            self.log(time, VectorProcessLogType::ProcessFailure { reason: "Upstream is empty" }).await;
                            self.time_to_next_event = None;
                        },
                        (None, _) => {
                            self.log(time, VectorProcessLogType::ProcessFailure { reason: "Upstream is not connected" }).await;
                            self.time_to_next_event = None;
                        },
                        (_, None) => {
                            self.log(time, VectorProcessLogType::ProcessFailure { reason: "Downstream is not connected" }).await;
                            self.time_to_next_event = None;
                        },
                        (_, Some(VectorStockState::Full {..} )) => {
                            self.log(time, VectorProcessLogType::ProcessFailure { reason: "Downstream is full" }).await;
                            self.time_to_next_event = None;
                        },
                    }
                },
                Some((time, _)) => {
                    self.time_to_next_event = Some(time);
                }
            }
        }
    }

    fn post_update_state<'a> (&'a mut self, notif_meta: &'a NotificationMetadata, cx: &'a mut nexosim::model::Context<Self>) -> impl Future<Output = ()> + Send + 'a where Self: Model {
        async move {
            self.set_previous_check_time(cx.time());
            match self.time_to_next_event {
                None => {},
                Some(time_until_next) => {
                    if time_until_next.is_zero() {
                        panic!("Time until next event is zero!");
                    } else {
                        let next_time = cx.time() + time_until_next;
                        cx.schedule_event(next_time, <Self as Process<T, T, f64, f64>>::update_state, notif_meta.clone()).unwrap();
                    };
                }
            };
        }
    }

    fn log<'a>(&'a mut self, time: MonotonicTime, details: VectorProcessLogType<T>) -> impl Future<Output = ()> + Send {
        async move {
            let log = VectorProcessLog {
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

impl<T, U: Clone + Send> VectorProcess<T, T, U> where T: VectorArithmetic<T, f64, f64> + Clone + Debug + Send + 'static, Self: Model {
    pub fn with_name(self, name: String) -> Self {
        VectorProcess {
            element_name: name,
            ..self
        }
    }

    pub fn with_type(self, element_type: String) -> Self {
        VectorProcess {
            element_type,
            ..self
        }
    }

    pub fn new() -> Self where T: Default {
        Self::default()
    }

    pub fn with_process_quantity_distr(self, process_quantity_distr: Distribution) -> Self {
        VectorProcess {
            process_quantity_distr,
            ..self
        }
    }

    pub fn with_process_quantity_distr_inplace(&mut self, process_quantity_distr: Distribution) {
        self.process_quantity_distr = process_quantity_distr;
    }

    pub fn with_process_time_distr(self, process_time_distr: Distribution) -> Self {
        VectorProcess {
            process_time_distr,
            ..self
        }
    }
}

pub struct VectorProcessLogger<T> where T: Send {
    pub name: String,
    pub buffer: EventQueue<VectorProcessLog<T>>,
}

impl<T> Logger for VectorProcessLogger<T> where VectorProcessLog<T>: Serialize, T: Send + 'static {
    type RecordType = VectorProcessLog<T>;
    fn get_name(&self) -> &String {
        &self.name
    }
    fn get_buffer(self) -> EventQueue<Self::RecordType> {
        self.buffer
    }
    fn new(name: String) -> Self {
        VectorProcessLogger {
            name,
            buffer: EventQueue::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum VectorProcessLogType<T> {
    ProcessStart { quantity: f64, vector: T },
    ProcessSuccess { quantity: f64, vector: T },
    ProcessFailure { reason: &'static str },
    CombineStart { quantity: f64, vectors: Vec<T> },
    CombineSuccess { quantity: f64, vector: T},
    CombineFailure { reason: &'static str },
    SplitStart { quantity: f64, vector: T },
    SplitSuccess { quantity: f64, vectors: Vec<T> },
    SplitFailure { reason: &'static str },
}

#[derive(Debug, Clone)]
pub struct VectorProcessLog<T> {
    pub time: String,
    pub event_id: u64,
    pub element_name: String,
    pub element_type: String,
    pub event: VectorProcessLogType<T>,
}

impl<T> Serialize for VectorProcessLog<T> where T: Serialize + Send + 'static {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("VectorProcessLog", 9)?;
        state.serialize_field("time", &self.time)?;
        state.serialize_field("event_id", &self.event_id)?;
        state.serialize_field("element_name", &self.element_name)?;
        state.serialize_field("element_type", &self.element_type)?;
        let (event_type, total, inflows, outflows, reason): (&str, Option<f64>, Option<String>, Option<String>, Option<String>);
        match &self.event {
            VectorProcessLogType::ProcessStart { quantity, vector } => {
                event_type = "ProcessStart";
                total = Some(*quantity);
                inflows = Some(serde_json::to_string(&vec![vector]).map_err(|e| serde::ser::Error::custom(e.to_string()))?);
                outflows = None;
                reason = None;
            },
            VectorProcessLogType::ProcessSuccess { quantity, vector } => {
                event_type = "ProcessSuccess";
                total = Some(*quantity);
                inflows = None;
                outflows = Some(serde_json::to_string(&vec![vector]).map_err(|e| serde::ser::Error::custom(e.to_string()))?);
                reason = None;
            },
            VectorProcessLogType::ProcessFailure { reason: r } => {
                event_type = "ProcessFailure";
                total = None;
                inflows = None;
                outflows = None;
                reason = Some(r.to_string());
            },
            VectorProcessLogType::CombineStart { quantity, vectors } => {
                event_type = "CombineStart";
                total = Some(*quantity);
                inflows = Some(serde_json::to_string(vectors).map_err(|e| serde::ser::Error::custom(e.to_string()))?);
                outflows = None;
                reason = None;
            },
            VectorProcessLogType::CombineSuccess { quantity, vector } => {
                event_type = "CombineSuccess";
                total = Some(*quantity);
                inflows = None;
                outflows = Some(serde_json::to_string(&vec![vector]).map_err(|e| serde::ser::Error::custom(e.to_string()))?);
                reason = None;
            },
            VectorProcessLogType::CombineFailure { reason: r } => {
                event_type = "CombineFailure";
                total = None;
                inflows = None;
                outflows = None;
                reason = Some(r.to_string());
            },
            VectorProcessLogType::SplitStart { quantity, vector } => {
                event_type = "SplitStart";
                total = Some(*quantity);
                inflows = Some(serde_json::to_string(&vec![vector]).map_err(|e| serde::ser::Error::custom(e.to_string()))?);
                outflows = None;
                reason = None;
            },
            VectorProcessLogType::SplitSuccess { quantity, vectors } => {
                event_type = "SplitSuccess";
                total = Some(*quantity);
                inflows = None;
                outflows = Some(serde_json::to_string(vectors).map_err(|e| serde::ser::Error::custom(e.to_string()))?);
                reason = None;
            },
            VectorProcessLogType::SplitFailure { reason: r } => {
                event_type = "SplitFailure";
                total = None;
                inflows = None;
                outflows = None;
                reason = Some(r.to_string());
            },
        }
        state.serialize_field("event_type", &event_type)?;
        state.serialize_field("total", &total)?;
        state.serialize_field("inflows", &inflows)?;
        state.serialize_field("outflows", &outflows)?;
        state.serialize_field("reason", &reason)?;
        state.end()
    }
}

/**
 * Combiner
 */

 /**
  * T: Resource type of upstream stock
  * U: Message type for pushing to downstream stock
  * V: Message type for withdrawing from upstream stock
  * M: Number of upstream stocks - number of stocks to combine from
  */
pub struct VectorCombiner<
    T: VectorArithmetic<T, f64, f64> + Clone + Debug + Send + 'static,
    U: Clone + Send + 'static,
    V: Clone + Send + 'static,
    const M: usize
> {
    pub element_name: String,
    pub element_type: String,
    pub req_upstreams: [Requestor<(), VectorStockState>; M],
    pub req_downstream: Requestor<(), VectorStockState>,
    pub withdraw_upstreams: [Requestor<(V, NotificationMetadata), T>; M],
    pub push_downstream: Output<(U, NotificationMetadata)>,
    pub process_state: Option<(Duration, Vec<T>)>,
    pub process_quantity_distr: Distribution,
    pub process_time_distr: Distribution,
    pub time_to_next_event: Option<Duration>,
    next_event_id: u64,
    pub log_emitter: Output<VectorProcessLog<T>>,
    pub previous_check_time: MonotonicTime,
    pub split_ratios: [f64; M],
}

impl<T: VectorArithmetic<T, f64, f64> + Clone + Debug + Send + 'static, const M: usize> Model for VectorCombiner<T, T, f64, M> {}

impl<
    T: VectorArithmetic<T, f64, f64> + Clone + Debug + Send + 'static,
    U: Clone + Send,
    V: Clone + Send,
    const M: usize
> VectorCombiner<T, U, V, M> {
    pub fn new() -> Self {
        VectorCombiner {
            element_name: String::new(),
            element_type: String::new(),
            req_upstreams: std::array::from_fn(|_| Requestor::default()),
            req_downstream: Requestor::default(),
            withdraw_upstreams: std::array::from_fn(|_| Requestor::default()),
            push_downstream: Output::default(),
            process_state: None,
            process_quantity_distr: Distribution::default(),
            process_time_distr: Distribution::default(),
            time_to_next_event: None,
            next_event_id: 0,
            log_emitter: Output::default(),
            previous_check_time: MonotonicTime::EPOCH,
            split_ratios: [1./(M as f64); M],
        }
    }

    pub fn with_name(self, name: String) -> Self {
        VectorCombiner {
            element_name: name,
            ..self
        }
    }

    pub fn with_process_quantity_distr(self, process_quantity_distr: Distribution) -> Self {
        VectorCombiner {
            process_quantity_distr,
            ..self
        }
    }

    pub fn with_process_time_distr(self, process_time_distr: Distribution) -> Self {
        VectorCombiner {
            process_time_distr,
            ..self
        }
    }
}

impl<T: VectorArithmetic<T, f64, f64> + Send + 'static + Clone + Debug + Default, const M: usize> Process<T, T, f64, f64> for VectorCombiner<T, T, f64, M> where Self: Model {
    type LogDetailsType = VectorProcessLogType<T>;
    
    fn set_previous_check_time(&mut self, time: MonotonicTime) {
        self.previous_check_time = time;
    }

    fn get_time_to_next_event(&mut self) -> &Option<Duration> {
        &self.time_to_next_event
    }

    fn set_time_to_next_event(&mut self, time: Option<Duration>) {
        self.time_to_next_event = time;
    }

    fn update_state_impl<'a>(&'a mut self, notif_meta: &'a NotificationMetadata, cx: &'a mut nexosim::model::Context<Self>) -> impl Future<Output = ()> + 'a where Self: Model {
        async move {
            let time = cx.time();

            match self.process_state.take() {
                Some((mut process_time_left, mut resources)) => {
                    let duration_since_prev_check = cx.time().duration_since(self.previous_check_time);
                    process_time_left = process_time_left.saturating_sub(duration_since_prev_check);
                    if process_time_left.is_zero() {
                        let mut total: T = Default::default();
                        while let Some(resource) = resources.pop() {
                            total.add(resource);
                        }
                        self.log(time, VectorProcessLogType::CombineSuccess { quantity: resources.iter().map(|x| x.total()).sum(), vector: total.clone() }).await;
                        self.push_downstream.send((total, NotificationMetadata {
                            time,
                            element_from: self.element_name.clone(),
                            message: format!("Pushing quantity {:?}", resources),
                        })).await;
                    } else {
                        self.process_state = Some((process_time_left, resources));
                    }
                },
                None => {}
            }
            match self.process_state {
                None => {
                    let iterators = join_all(self.req_upstreams.iter_mut().map(|req| req.send(()))).await;
                    let us_states: Vec<VectorStockState> = iterators.into_iter().flatten().collect();
                    let all_us_available: Option<bool>;
                    if us_states.len() < M {
                        all_us_available = None;
                    } else {
                        all_us_available = Some(us_states.iter().all(|state| {
                            matches!(state, VectorStockState::Normal {..} | VectorStockState::Full {..})
                        }));
                    }
                    let ds_state = self.req_downstream.send(()).await.next();
                    match (all_us_available, ds_state) {
                        (
                            Some(true),
                            Some(VectorStockState::Empty {..}) | Some(VectorStockState::Normal {..}),
                        ) => {
                            let process_quantity = self.process_quantity_distr.sample();
                            let withdraw_iterators = join_all(self.withdraw_upstreams.iter_mut().map(|req| {
                                req.send((process_quantity, NotificationMetadata {
                                    time, 
                                    element_from: self.element_name.clone(),
                                    message: format!("Withdrawing quantity {:?}", process_quantity),
                                }))
                            })).await;
                            let withdrawn: Vec<T> = withdraw_iterators.into_iter().filter_map(|mut x| x.next()).collect();
                            let process_duration_secs = self.process_time_distr.sample();
                            self.process_state = Some((Duration::from_secs_f64(process_duration_secs), withdrawn.clone()));
                            self.log(time, VectorProcessLogType::CombineStart { quantity: process_quantity, vectors: withdrawn }).await;
                            self.time_to_next_event = Some(Duration::from_secs_f64(process_duration_secs));
                        },
                        (Some(false), _) => {
                            self.log(time, VectorProcessLogType::ProcessFailure { reason: "At least one upstream is empty" }).await;
                            self.time_to_next_event = None;
                        },
                        (None, _) => {
                            self.log(time, VectorProcessLogType::ProcessFailure { reason: "No upstreams are connected" }).await;
                            self.time_to_next_event = None;
                        },
                        (_, None) => {
                            self.log(time, VectorProcessLogType::ProcessFailure { reason: "Downstream is not connected" }).await;
                            self.time_to_next_event = None;
                        },
                        (_, Some(VectorStockState::Full {..} )) => {
                            self.log(time, VectorProcessLogType::ProcessFailure { reason: "Downstream is full" }).await;
                            self.time_to_next_event = None;
                        },
                    }
                },
                Some((time, _)) => {
                    self.time_to_next_event = Some(time);
                }
            }
        }
    }

    fn post_update_state<'a> (&'a mut self, notif_meta: &'a NotificationMetadata, cx: &'a mut nexosim::model::Context<Self>) -> impl Future<Output = ()> + Send + 'a where Self: Model {
        async move {
            self.set_previous_check_time(cx.time());
            match self.time_to_next_event {
                None => {},
                Some(time_until_next) => {
                    if time_until_next.is_zero() {
                        panic!("Time until next event is zero!");
                    } else {
                        let next_time = cx.time() + time_until_next;
                        cx.schedule_event(next_time, <Self as Process<T, T, f64, f64>>::update_state, notif_meta.clone()).unwrap();
                    };
                }
            };
        }
    }

    fn log<'a>(&'a mut self, time: MonotonicTime, details: Self::LogDetailsType) -> impl Future<Output = ()> + Send {
        async move {
            let log = VectorProcessLog {
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


/**
 * Splitter
 */

 /**
  * T: Resource type of upstream stock
  * U: Message type for pushing to downstream stock
  * V: Message type for withdrawing from upstream stock
  * N: Number of downstream stocks - number of stocks to split into
  */

pub struct VectorSplitter<
    T: VectorArithmetic<T, f64, f64> + Clone + Debug + Send + 'static,
    U: Clone + Send + 'static,
    V: Clone + Send + 'static,
    const N: usize
> {
    pub element_name: String,
    pub element_type: String,
    pub req_upstream: Requestor<(), VectorStockState>,
    pub req_downstreams: [Requestor<(), VectorStockState>; N],
    pub withdraw_upstream: Requestor<(V, NotificationMetadata), T>,
    pub push_downstreams: [Output<(U, NotificationMetadata)>; N],
    pub process_state: Option<(Duration, U)>,
    pub process_quantity_distr: Distribution,
    pub process_time_distr: Distribution,
    pub time_to_next_event: Option<Duration>,
    next_event_id: u64,
    pub log_emitter: Output<VectorProcessLog<T>>,
    pub previous_check_time: MonotonicTime,
    pub split_ratios: [f64; N],
}

impl<T: VectorArithmetic<T, f64, f64> + Send + 'static + Clone + Debug + Default, const N: usize> VectorSplitter<T, T, f64, N> {
    pub fn new() -> Self {
        Default::default()
    }
    
    pub fn with_name(self, name: String) -> Self {
        VectorSplitter {
            element_name: name,
            ..self
        }
    }

    pub fn with_process_quantity_distr(self, process_quantity_distr: Distribution) -> Self {
        VectorSplitter {
            process_quantity_distr,
            ..self
        }
    }

    pub fn with_process_time_distr(self, process_time_distr: Distribution) -> Self {
        VectorSplitter {
            process_time_distr,
            ..self
        }
    }
}

impl<T: VectorArithmetic<T, f64, f64> + Send + 'static + Clone + Debug, U: Clone + Send, V: Clone + Send, const N: usize> Default for VectorSplitter<T, U, V, N> {
    fn default() -> Self {
        VectorSplitter {
            element_name: String::new(),
            element_type: String::new(),
            req_upstream: Requestor::default(),
            req_downstreams: std::array::from_fn(|_| Requestor::default()),
            withdraw_upstream: Requestor::default(),
            push_downstreams: std::array::from_fn(|_| Output::default()),
            process_state: None,
            process_quantity_distr: Distribution::default(),
            process_time_distr: Distribution::default(),
            time_to_next_event: None,
            next_event_id: 0,
            log_emitter: Output::default(),
            previous_check_time: MonotonicTime::EPOCH,
            split_ratios: [1./(N as f64); N],
        }
    }
}

impl<T: VectorArithmetic<T, f64, f64> + Send + Clone + Debug, V: Clone + Debug + Send, const N: usize> Model for VectorSplitter<T, T, V, N> {}

impl<T: VectorArithmetic<T, f64, f64> + Send + 'static + Clone + Debug + Default, const N: usize> Process<T, T, f64, f64> for VectorSplitter<T, T, f64, N> where Self: Model {
    type LogDetailsType = VectorProcessLogType<T>;
    
    fn set_previous_check_time(&mut self, time: MonotonicTime) {
        self.previous_check_time = time;
    }

    fn get_time_to_next_event(&mut self) -> &Option<Duration> {
        &self.time_to_next_event
    }

    fn set_time_to_next_event(&mut self, time: Option<Duration>) {
        self.time_to_next_event = time;
    }

    fn update_state_impl<'a>(&'a mut self, notif_meta: &'a NotificationMetadata, cx: &'a mut nexosim::model::Context<Self>) -> impl Future<Output = ()> + 'a where Self: Model {
        async move {
            let time = cx.time();

            match self.process_state.take() {
                Some((mut process_time_left, resource)) => {
                    let duration_since_prev_check = cx.time().duration_since(self.previous_check_time);
                    process_time_left = process_time_left.saturating_sub(duration_since_prev_check);

                    if process_time_left.is_zero() {
    
                        let split_resources = self.split_ratios.iter().map(|ratio| {
                            let quantity = resource.total() * ratio;
                            resource.clone().subtract(quantity)
                        }).collect::<Vec<_>>();
                        
                        self.log(time, VectorProcessLogType::SplitSuccess { quantity: resource.total(), vectors: split_resources.clone() }).await;

                        join_all(self.push_downstreams.iter_mut().zip(split_resources).map(|(push, resource)| {
                            push.send((resource.clone(), NotificationMetadata {
                                time,
                                element_from: self.element_name.clone(),
                                message: format!("Pushing quantity {:?}", resource),
                            }))
                        })).await;
                    } else {
                        self.process_state = Some((process_time_left, resource));
                    }
                },
                None => {}
            }
            match self.process_state {
                None => {
                    let us_state = self.req_upstream.send(()).await.next();
                    let ds_states = join_all(self.req_downstreams.iter_mut().map(|req| req.send(()))).await.iter_mut().map(|x| {
                        x.next()
                    }).collect::<Vec<Option<VectorStockState>>>();
                    let all_ds_available: Option<bool>;
                    if ds_states.len() < N {
                        all_ds_available = None;
                    } else {
                        all_ds_available = Some(ds_states.iter().all(|state| {
                            matches!(state, Some(VectorStockState::Normal {..}) | Some(VectorStockState::Empty {..}))
                        }));
                    }

                    match (us_state, all_ds_available) {
                        (
                            Some(VectorStockState::Full {..}) | Some(VectorStockState::Normal {..}),
                            Some(true),
                        ) => {
                            let process_quantity = self.process_quantity_distr.sample();
                            let withdrawn = self.withdraw_upstream.send((process_quantity, NotificationMetadata {
                                time,
                                element_from: self.element_name.clone(),
                                message: format!("Withdrawing quantity {:?}", process_quantity),
                            })).await.next().unwrap();
                            let process_duration_secs = self.process_time_distr.sample();
                            self.process_state = Some((Duration::from_secs_f64(process_duration_secs), withdrawn.clone()));
                            self.log(time, VectorProcessLogType::SplitStart { quantity: process_quantity, vector: withdrawn }).await;
                            self.time_to_next_event = Some(Duration::from_secs_f64(process_duration_secs));
                        },
                        (_, Some(false)) => {
                            self.log(time, VectorProcessLogType::ProcessFailure { reason: "At least one downstream is full" }).await;
                            self.time_to_next_event = None;
                        },
                        (_, None) => {
                            self.log(time, VectorProcessLogType::ProcessFailure { reason: "No downstreams are connected" }).await;
                            self.time_to_next_event = None;
                        },
                        (None, _) => {
                            self.log(time, VectorProcessLogType::ProcessFailure { reason: "Upstream is not connected" }).await;
                            self.time_to_next_event = None;
                        },
                        (Some(VectorStockState::Empty {..} ), _) => {
                            self.log(time, VectorProcessLogType::ProcessFailure { reason: "Upstream is empty" }).await;
                            self.time_to_next_event = None;
                        },
                    }
                },
                Some((time, _)) => {
                    self.time_to_next_event = Some(time);
                }
            }
        }
    }

    fn post_update_state<'a> (&'a mut self, notif_meta: &'a NotificationMetadata, cx: &'a mut nexosim::model::Context<Self>) -> impl Future<Output = ()> + Send + 'a where Self: Model {
        async move {
            self.set_previous_check_time(cx.time());
            match self.time_to_next_event {
                None => {},
                Some(time_until_next) => {
                    if time_until_next.is_zero() {
                        panic!("Time until next event is zero!");
                    } else {
                        let next_time = cx.time() + time_until_next;
                        cx.schedule_event(next_time, <Self as Process<T, T, f64, f64>>::update_state, notif_meta.clone()).unwrap();
                    };
                }
            };
        }
    }

    fn log<'a>(&'a mut self, time: MonotonicTime, details: Self::LogDetailsType) -> impl Future<Output = ()> + Send {
        async move {
            let log = VectorProcessLog {
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


/**
 * Source
 */

/**
 * T: Resource type of upstream stock
 * U: Message type for pushing to downstream stock
 */

pub struct VectorSource<
    T: VectorArithmetic<T, f64, f64> + Clone + Debug + Send + 'static,
    U: Clone + Send + 'static>
{
    pub element_name: String,
    pub element_type: String,
    pub req_downstream: Requestor<(), VectorStockState>,
    pub push_downstream: Output<(U, NotificationMetadata)>,
    pub process_state: Option<(Duration, T)>,
    pub process_quantity_distr: Distribution,
    pub process_time_distr: Distribution,
    pub source_vector: T,
    pub time_to_next_event: Option<Duration>,
    next_event_id: u64,
    pub log_emitter: Output<VectorProcessLog<T>>,
    pub previous_check_time: MonotonicTime,
}

impl<
    T: VectorArithmetic<T, f64, f64> + Clone + Debug + Default + Send,
    U: Clone + Send
> Default for VectorSource<T, U>  {
    fn default() -> Self {
        VectorSource {
            element_name: String::new(),
            element_type: String::new(),
            req_downstream: Requestor::default(),
            push_downstream: Output::default(),
            process_state: None,
            process_quantity_distr: Distribution::default(),
            process_time_distr: Distribution::default(),
            source_vector: T::default(),
            time_to_next_event: None,
            next_event_id: 0,
            log_emitter: Output::default(),
            previous_check_time: MonotonicTime::EPOCH,
        }
    }
}

impl<T: VectorArithmetic<T, f64, f64> + Send + 'static + Clone + Debug + Default> Model for VectorSource<T, T> {}

impl<T: VectorArithmetic<T, f64, f64> + Send + 'static + Clone + Debug + Default> VectorSource<T, T> {
    pub fn new() -> Self {
        Default::default()
    }
    
    pub fn with_name(self, name: String) -> Self {
        VectorSource {
            element_name: name,
            ..self
        }
    }

    pub fn with_process_quantity_distr(self, process_quantity_distr: Distribution) -> Self {
        VectorSource {
            process_quantity_distr,
            ..self
        }
    }

    pub fn with_process_time_distr(self, process_time_distr: Distribution) -> Self {
        VectorSource {
            process_time_distr,
            ..self
        }
    }

    pub fn with_source_vector(self, source_vector: T) -> Self {
        VectorSource {
            source_vector,
            ..self
        }
    }
}

impl<T: VectorArithmetic<T, f64, f64> + Send + 'static + Clone + Debug> Process<T, T, f64, f64> for VectorSource<T, T> where Self: Model {
    type LogDetailsType = VectorProcessLogType<T>;
    
    fn set_previous_check_time(&mut self, time: MonotonicTime) {
        self.previous_check_time = time;
    }

    fn get_time_to_next_event(&mut self) -> &Option<Duration> {
        &self.time_to_next_event
    }

    fn set_time_to_next_event(&mut self, time: Option<Duration>) {
        self.time_to_next_event = time;
    }

    fn update_state_impl<'a>(&'a mut self, notif_meta: &'a NotificationMetadata, cx: &'a mut nexosim::model::Context<Self>) -> impl Future<Output = ()> + 'a where Self: Model {
        async move {
            let time = cx.time();

            match self.process_state.take() {
                Some((mut process_time_left, resource)) => {
                    let duration_since_prev_check = cx.time().duration_since(self.previous_check_time);
                    process_time_left = process_time_left.saturating_sub(duration_since_prev_check);
                    if process_time_left.is_zero() {
                        self.log(time, VectorProcessLogType::ProcessSuccess { quantity: resource.total(), vector: resource.clone() }).await;
                        self.push_downstream.send((resource.clone(), NotificationMetadata {
                            time,
                            element_from: self.element_name.clone(),
                            message: format!("Pushing quantity {:?}", resource),
                        })).await;
                    } else {
                        self.process_state = Some((process_time_left, resource));
                    }
                },
                None => {}
            }
            match self.process_state {
                None => {
                    let ds_state = self.req_downstream.send(()).await.next();
                    match ds_state {
                        Some(VectorStockState::Full {..}) => {
                            self.log(time, VectorProcessLogType::ProcessFailure { reason: "Downstream is full" }).await;
                            self.time_to_next_event = None;
                        },
                        Some(VectorStockState::Normal {..}) | Some(VectorStockState::Empty {..}) => {
                            let process_quantity = self.process_quantity_distr.sample();
                            if self.source_vector.total() <= 0. {
                                panic!("Source vector has total 0 or negative ({}), cannot process!", self.source_vector.total());
                            }
                            let mut created = self.source_vector.clone();
                            
                            created.multiply(process_quantity / created.total());
                            let process_duration_secs = self.process_time_distr.sample();
                            self.process_state = Some((Duration::from_secs_f64(process_duration_secs), created.clone()));
                            self.log(time, VectorProcessLogType::ProcessStart { quantity: process_quantity, vector: created }).await;
                            self.time_to_next_event = Some(Duration::from_secs_f64(process_duration_secs));
                        },
                        None => {
                            self.log(time, VectorProcessLogType::ProcessFailure { reason: "Downstream is not connected" }).await;
                            self.time_to_next_event = None;
                        },
                    }
                },
                Some((time, _)) => {
                    self.time_to_next_event = Some(time);
                }
            }
        }
    }

    fn post_update_state<'a> (&'a mut self, notif_meta: &'a NotificationMetadata, cx: &'a mut nexosim::model::Context<Self>) -> impl Future<Output = ()> + Send + 'a where Self: Model {
        async move {
            self.set_previous_check_time(cx.time());
            match self.time_to_next_event {
                None => {},
                Some(time_until_next) => {
                    if time_until_next.is_zero() {
                        panic!("Time until next event is zero!");
                    } else {
                        let next_time = cx.time() + time_until_next;
                        cx.schedule_event(next_time, <Self as Process<T, T, f64, f64>>::update_state, notif_meta.clone()).unwrap();
                    };
                }
            };
        }
    }

    fn log<'a>(&'a mut self, time: MonotonicTime, details: Self::LogDetailsType) -> impl Future<Output = ()> + Send {
        async move {
            let log = VectorProcessLog {
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

/**
 * Sink
 */

/**
 * T: Resource type of upstream stock
 * V: Message type for pushing to downstream stock
 */

pub struct VectorSink<T: VectorArithmetic<T, f64, f64> + Clone + Debug + Send + 'static> {
    pub element_name: String,
    pub element_type: String,
    pub req_upstream: Requestor<(), VectorStockState>,
    pub withdraw_upstream: Requestor<(f64, NotificationMetadata), T>,
    pub process_state: Option<(Duration, T)>,
    pub process_quantity_distr: Distribution,
    pub process_time_distr: Distribution,
    pub time_to_next_event: Option<Duration>,
    next_event_id: u64,
    pub log_emitter: Output<VectorProcessLog<T>>,
    pub previous_check_time: MonotonicTime,
}

impl<T: VectorArithmetic<T, f64, f64> + Send + 'static + Clone + Debug + Default> Model for VectorSink<T> {}

impl<T: VectorArithmetic<T, f64, f64> + Clone + Send + Debug + Default + 'static> Default for VectorSink<T> {
    fn default() -> Self {
        VectorSink {
            element_name: String::new(),
            element_type: String::new(),
            req_upstream: Requestor::default(),
            withdraw_upstream: Requestor::default(),
            process_state: None,
            process_quantity_distr: Distribution::default(),
            process_time_distr: Distribution::default(),
            time_to_next_event: None,
            next_event_id: 0,
            log_emitter: Output::default(),
            previous_check_time: MonotonicTime::EPOCH,
        }
    }
}

impl<T: VectorArithmetic<T, f64, f64> + Send + 'static + Clone + Debug + Default> VectorSink<T> {
    pub fn new() -> Self {
        Default::default()
    }
    
    pub fn with_name(self, name: String) -> Self {
        VectorSink {
            element_name: name,
            ..self
        }
    }

    pub fn with_process_quantity_distr(self, process_quantity_distr: Distribution) -> Self {
        VectorSink {
            process_quantity_distr,
            ..self
        }
    }

    pub fn with_process_time_distr(self, process_time_distr: Distribution) -> Self {
        VectorSink {
            process_time_distr,
            ..self
        }
    }
}

impl<T: VectorArithmetic<T, f64, f64> + Send + 'static + Clone + Debug + Default> Process<T, T, f64, f64> for VectorSink<T> where Self: Model {
    type LogDetailsType = VectorProcessLogType<T>;
    
    fn set_previous_check_time(&mut self, time: MonotonicTime) {
        self.previous_check_time = time;
    }

    fn get_time_to_next_event(&mut self) -> &Option<Duration> {
        &self.time_to_next_event
    }

    fn set_time_to_next_event(&mut self, time: Option<Duration>) {
        self.time_to_next_event = time;
    }

    fn update_state_impl<'a>(&'a mut self, notif_meta: &'a NotificationMetadata, cx: &'a mut nexosim::model::Context<Self>) -> impl Future<Output = ()> + 'a where Self: Model {
        async move {
            let time = cx.time();

            match self.process_state.take() {
                Some((mut process_time_left, resource)) => {
                    let duration_since_prev_check = cx.time().duration_since(self.previous_check_time);
                    process_time_left = process_time_left.saturating_sub(duration_since_prev_check);
                    if process_time_left.is_zero() {
                        self.log(time, VectorProcessLogType::ProcessSuccess { quantity: resource.total(), vector: resource.clone() }).await;
                    } else {
                        self.process_state = Some((process_time_left, resource));
                    }
                },
                None => {}
            }
            match self.process_state {
                None => {
                    let us_state = self.req_upstream.send(()).await.next();
                    match us_state {
                        Some(VectorStockState::Normal {..}) | Some(VectorStockState::Full {..}) => {
                            let process_quantity = self.process_quantity_distr.sample();
                            let withdrawn = self.withdraw_upstream.send((process_quantity, NotificationMetadata {
                                time,
                                element_from: self.element_name.clone(),
                                message: format!("Withdrawing quantity {:?}", process_quantity),
                            })).await.next().unwrap();
                            let process_duration_secs = self.process_time_distr.sample();
                            self.process_state = Some((Duration::from_secs_f64(process_duration_secs), withdrawn.clone()));
                            self.log(time, VectorProcessLogType::ProcessStart { quantity: process_quantity, vector: withdrawn }).await;
                            self.time_to_next_event = Some(Duration::from_secs_f64(process_duration_secs));
                        },
                        Some(VectorStockState::Empty {..}) => {
                        }
                        None => {
                            self.log(time, VectorProcessLogType::ProcessFailure { reason: "Upstream is not connected" }).await;
                            self.time_to_next_event = None;
                        },
                        _ => {
                            self.log(time, VectorProcessLogType::ProcessFailure { reason: "Upstream is empty" }).await;
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

    fn log<'a>(&'a mut self, time: MonotonicTime, details: Self::LogDetailsType) -> impl Future<Output = ()> + Send {
        async move {
            let log = VectorProcessLog {
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