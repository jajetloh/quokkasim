use nexosim::{model::Model, ports::{EventBuffer, Output, Requestor}};
use serde::{ser::SerializeStruct, Serialize};
use tai_time::MonotonicTime;
use std::{fmt::Debug, time::Duration};

use crate::{core::{Distribution, NotificationMetadata, StateEq}, new_core::{Process, Stock}, prelude::{SubtractParts, Vector3, VectorArithmetic}};
use crate::new_core::Logger;

/**
 * Stock
 */

#[derive(Debug, Clone)]
pub enum NewVectorStockState {
    Empty { occupied: f64, empty: f64 },
    Normal { occupied: f64, empty: f64 },
    Full { occupied: f64, empty: f64 },
}

impl NewVectorStockState {
    pub fn get_name(&self) -> String {
        match self {
            NewVectorStockState::Empty { .. } => "Empty".to_string(),
            NewVectorStockState::Normal { .. } => "Normal".to_string(),
            NewVectorStockState::Full { .. } => "Full".to_string(),
        }
    }
}

impl StateEq for NewVectorStockState {
    fn is_same_state(&self, other: &Self) -> bool {
        match (self, other) {
            (NewVectorStockState::Empty { .. }, NewVectorStockState::Empty { .. }) => true,
            (NewVectorStockState::Normal { .. }, NewVectorStockState::Normal { .. }) => true,
            (NewVectorStockState::Full { .. }, NewVectorStockState::Full { .. }) => true,
            _ => false,
        }
    }
}

pub struct NewVectorStock<T: VectorArithmetic + Clone + Debug + Send + 'static> {
    pub element_name: String,
    pub element_type: String,
    pub vector: T,
    pub log_emitter: Output<NewVectorStockLog<T>>,
    pub state_emitter: Output<NotificationMetadata>,
    pub low_capacity: f64,
    pub max_capacity: f64,
    pub prev_state: Option<NewVectorStockState>,
}
impl<T: VectorArithmetic + Clone + Debug + Default + Send> Default for NewVectorStock<T> {
    fn default() -> Self {
        NewVectorStock {
            element_name: String::new(),
            element_type: String::new(),
            vector: Default::default(),
            low_capacity: 0.0,
            max_capacity: 0.0,
            log_emitter: Output::default(),
            state_emitter: Output::default(),
            prev_state: None,
        }
    }
}

impl<T: VectorArithmetic + Clone + Debug + Send> Stock<T, T, f64> for NewVectorStock<T> where Self: Model {

    type StockState = NewVectorStockState;
    // type LogDetailsType = NewVectorStockLog<T>;

    fn get_state(&mut self) -> Self::StockState {
        let occupied = self.vector.total();
        let empty = self.max_capacity - occupied;
        if empty <= 0.0 {
            NewVectorStockState::Full { occupied, empty }
        } else if occupied < self.low_capacity {
            NewVectorStockState::Empty { occupied, empty }
        } else {
            NewVectorStockState::Normal { occupied, empty }
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
            let added = self.vector.add(&payload.0);
            self.vector = added.clone();
        }
    }

    fn remove_impl<'a>(
        &'a mut self,
        data: &'a (f64, NotificationMetadata),
        cx: &'a mut ::nexosim::model::Context<Self>
    ) -> impl Future<Output=T> + 'a {
        async move {
            self.prev_state = Some(self.get_state());
            let SubtractParts { subtracted, remaining } = self.vector.subtract_parts(data.0.clone());
            self.vector = remaining;
            subtracted
        }
    }

    fn emit_change(&mut self, payload: NotificationMetadata, cx: &mut nexosim::model::Context<Self>) {
        self.state_emitter.send(payload);
    }

    fn log(&mut self, time: MonotonicTime, log_type: String) -> impl Future<Output=()> + Send {
        async move {
            let log = NewVectorStockLog {
                time: time.to_chrono_date_time(0).unwrap().to_string(),
                event_id: "01234".into(),
                element_name: self.element_name.clone(),
                element_type: self.element_type.clone(),
                log_type,
                state: self.get_state(),
                vector: self.vector.clone(),
            };
            self.log_emitter.send(log).await;
        }
    }
}

impl<T: VectorArithmetic + Clone + Debug + Send> NewVectorStock<T> where Self: Model {
    pub fn get_state(&mut self) -> NewVectorStockState {
        let occupied = self.vector.total();
        let empty = self.max_capacity - occupied;
        if empty <= 0.0 {
            NewVectorStockState::Full { occupied, empty }
        } else if occupied < self.low_capacity {
            NewVectorStockState::Empty { occupied, empty }
        } else {
            NewVectorStockState::Normal { occupied, empty }
        }
    }
}

impl Model for NewVectorStock<f64> {}
impl Model for NewVectorStock<Vector3> {}

pub struct NewVectorStockLogger<T> {
    pub name: String,
    pub buffer: EventBuffer<NewVectorStockLog<T>>,
}

#[derive(Debug, Clone)]
pub struct NewVectorStockLog<T> {
    pub time: String,
    pub event_id: String,
    pub element_name: String,
    pub element_type: String,
    pub log_type: String,
    pub state: NewVectorStockState,
    pub vector: T,
}

impl Serialize for NewVectorStockLog<f64> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("NewVectorStockLog", 6)?;
        state.serialize_field("time", &self.time)?;
        state.serialize_field("event_id", &self.event_id)?;
        state.serialize_field("element_name", &self.element_name)?;
        state.serialize_field("element_type", &self.element_type)?;
        state.serialize_field("log_type", &self.log_type)?;
        state.serialize_field("state", &self.state.get_name())?;
        state.serialize_field("value", &self.vector)?;
        state.end()
    }
}

impl Serialize for NewVectorStockLog<Vector3> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("NewVectorStockLog", 6)?;
        state.serialize_field("time", &self.time)?;
        state.serialize_field("event_id", &self.event_id)?;
        state.serialize_field("element_name", &self.element_name)?;
        state.serialize_field("element_type", &self.element_type)?;
        state.serialize_field("log_type", &self.log_type)?;
        state.serialize_field("state", &self.state.get_name())?;
        state.serialize_field("x0", &self.vector.values[0])?;
        state.serialize_field("x1", &self.vector.values[1])?;
        state.serialize_field("x2", &self.vector.values[2])?;
        state.end()
    }
}

impl Logger for NewVectorStockLogger<f64> {
    type RecordType = NewVectorStockLog<f64>;
    fn get_name(&self) -> &String {
        &self.name
    }
    fn get_buffer(self) -> EventBuffer<Self::RecordType> {
        self.buffer
    }
    fn new(name: String, buffer_size: usize) -> Self {
        NewVectorStockLogger {
            name,
            buffer: EventBuffer::with_capacity(buffer_size),
        }
    }
}

impl Logger for NewVectorStockLogger<Vector3> {
    type RecordType = NewVectorStockLog<Vector3>;
    fn get_name(&self) -> &String {
        &self.name
    }
    fn get_buffer(self) -> EventBuffer<Self::RecordType> {
        self.buffer
    }
    fn new(name: String, capacity: usize) -> Self {
        NewVectorStockLogger {
            name,
            buffer: EventBuffer::with_capacity(capacity),
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
  */
pub struct NewVectorProcess<T: VectorArithmetic + Clone + Debug + Send + 'static, U: Clone + Send + 'static, V: Clone + Send + 'static> {
    pub element_name: String,
    pub element_type: String,
    pub req_upstream: Requestor<(), NewVectorStockState>,
    pub req_downstream: Requestor<(), NewVectorStockState>,
    pub withdraw_upstream: Requestor<(V, NotificationMetadata), T>,
    pub push_downstream: Output<(U, NotificationMetadata)>,
    pub process_quantity_distr: Distribution,
    pub process_time_distr: Distribution,
    pub time_to_next_event_counter: Option<Duration>,
    next_event_id: u64,
    pub log_emitter: Output<NewVectorProcessLog<T>>,
    pub previous_check_time: MonotonicTime,
}
impl<T: VectorArithmetic + Clone + Debug + Default + Send, U: Clone + Send, V: Clone + Send> Default for NewVectorProcess<T, U, V> {
    fn default() -> Self {
        NewVectorProcess {
            element_name: String::new(),
            element_type: String::new(),
            req_upstream: Requestor::default(),
            req_downstream: Requestor::default(),
            withdraw_upstream: Requestor::default(),
            push_downstream: Output::default(),
            process_quantity_distr: Distribution::default(),
            process_time_distr: Distribution::default(),
            time_to_next_event_counter: None,
            next_event_id: 0,
            log_emitter: Output::default(),
            previous_check_time: MonotonicTime::EPOCH,
        }
    }
}

impl<T: VectorArithmetic + Send + 'static + Clone + Debug, U: Clone + Send, V: Clone + Send> Model for NewVectorProcess<T, U, V> {}

impl<T: VectorArithmetic + Send + 'static + Clone + Debug> Process<T> for NewVectorProcess<T, T, f64> where Self: Model {

    type LogDetailsType = NewVectorProcessLogType<T>;

    fn get_time_to_next_event(&mut self) -> &Option<Duration> {
        &self.time_to_next_event_counter
    }
    fn set_time_to_next_event(&mut self, time: Option<Duration>) {
        self.time_to_next_event_counter = time;
    }
    fn set_previous_check_time(&mut self, time: MonotonicTime) {
        self.previous_check_time = time;
    }
    fn update_state_impl<'a> (&'a mut self, notif_meta: &NotificationMetadata, cx: &'a mut nexosim::model::Context<Self>) -> impl Future<Output = ()> + 'a where Self: Model {
        async move {
            let time = cx.time();
            println!("Update state: {:?}", time);
            let us_state = self.req_upstream.send(()).await.next();
            let ds_state = self.req_downstream.send(()).await.next();
            match (&us_state, &ds_state) {
                (
                    Some(NewVectorStockState::Normal {..}) | Some(NewVectorStockState::Full {..}),
                    Some(NewVectorStockState::Empty {..}) | Some(NewVectorStockState::Normal {..}),
                ) => {
                    let process_quantity = self.process_quantity_distr.sample();
                    let moved = self.withdraw_upstream.send((process_quantity, NotificationMetadata {
                        time,
                        element_from: self.element_name.clone(),
                        message: format!("Withdrawing quantity {:?}", process_quantity),
                    })).await.next().unwrap();

                    self.push_downstream.send((moved.clone(), NotificationMetadata {
                        time,
                        element_from: self.element_name.clone(),
                        message: format!("Depositing quantity {:?} ({:?})", process_quantity, moved),
                    })).await;

                    self.log(time, NewVectorProcessLogType::ProcessSuccess { quantity: process_quantity, vector: moved }).await;
                    self.time_to_next_event_counter = Some(Duration::from_secs_f64(self.process_time_distr.sample()));
                },
                (Some(NewVectorStockState::Empty {..} ), _) => {
                    self.log(time, NewVectorProcessLogType::ProcessFailure { reason: "Upstream is empty" }).await;
                    self.time_to_next_event_counter = None;
                },
                (None, _) => {
                    self.log(time, NewVectorProcessLogType::ProcessFailure { reason: "Upstream is not connected" }).await;
                    self.time_to_next_event_counter = None;
                },
                (_, None) => {
                    self.log(time, NewVectorProcessLogType::ProcessFailure { reason: "Downstream is not connected" }).await;
                    self.time_to_next_event_counter = None;
                },
                (_, Some(NewVectorStockState::Full {..} )) => {
                    self.log(time, NewVectorProcessLogType::ProcessFailure { reason: "Downstream is full" }).await;
                    self.time_to_next_event_counter = None;
                },
            }
        }
    }

    fn post_update_state<'a> (&'a mut self, notif_meta: &'a NotificationMetadata, cx: &'a mut nexosim::model::Context<Self>) -> impl Future<Output = ()> + Send + 'a where Self: Model {
        // async move {
        //     cx.schedule_event(MonotonicTime::EPOCH, <Self as Process<f64>>::update_state, notif_meta.clone()).unwrap();
        //     // cx.schedule_event(next_time, <Self as Process<f64>>::post_update_state, notif_meta.clone()).unwrap();
        // }
        async move {
            self.set_previous_check_time(cx.time());
            match self.time_to_next_event_counter {
                None => {},
                Some(time_until_next) => {
                    if time_until_next.is_zero() {
                        panic!("Time until next event is zero!");
                    } else {
                        let next_time = cx.time() + time_until_next;
                        cx.schedule_event(next_time, <Self as Process<T>>::update_state, notif_meta.clone()).unwrap();
                    };
                }
            };
        }
    }

    fn log<'a>(&'a mut self, time: MonotonicTime, details: NewVectorProcessLogType<T>) -> impl Future<Output = ()> + Send {
        async move {
            let log = NewVectorProcessLog {
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

// impl Process<Vector3> for NewVectorProcess<Vector3> {}

// impl<T: VectorArithmetic + Clone + Debug + Send> NewVectorProcess<T> where Self: Model {
// impl<T: VectorArithmetic + Clone + Debug + Send> NewVectorProcess<T> where Self: Model {
//     pub fn check_update_state<'a>(
//         &'a mut self,
//         notif_meta: NotificationMetadata,
//         cx: &'a mut ::nexosim::model::Context<Self>,
//     ) -> impl Future<Output = ()> + Send {
//         async move {
//             let time = cx.time();
//             let us_state = self.req_upstream.send(()).await.next();
//             let ds_state = self.req_downstream.send(()).await.next();
//             match (&us_state, &ds_state) {
//                 (
//                     Some(NewVectorStockState::Normal {..}) | Some(NewVectorStockState::Full {..}),
//                     Some(NewVectorStockState::Empty {..}) | Some(NewVectorStockState::Normal {..}),
//                 ) => {
//                     let process_quantity = self.process_quantity_distr.sample();
//                     let moved = self.withdraw_upstream.send((process_quantity, NotificationMetadata {
//                         time,
//                         element_from: self.element_name.clone(),
//                         message: format!("Withdrawing quantity {:?}", process_quantity),
//                     })).await.next().unwrap();

//                     self.push_downstream.send((moved.clone(), NotificationMetadata {
//                         time,
//                         element_from: self.element_name.clone(),
//                         message: format!("Depositing quantity {:?} ({:?})", process_quantity, moved),
//                     })).await;

//                     self.log(time, NewVectorProcessLogType::ProcessSuccess { quantity: process_quantity, vector: moved }).await;
//                 },
//                 (Some(NewVectorStockState::Empty {..} ), _) => {
//                     self.log(time, NewVectorProcessLogType::ProcessFailure { reason: "Upstream is empty" }).await;
//                 },
//                 (None, _) => {
//                     self.log(time, NewVectorProcessLogType::ProcessFailure { reason: "Upstream is not connected" }).await;
//                 },
//                 (_, None) => {
//                     self.log(time, NewVectorProcessLogType::ProcessFailure { reason: "Downstream is not connected" }).await;
//                 },
//                 (_, Some(NewVectorStockState::Full {..} )) => {
//                     self.log(time, NewVectorProcessLogType::ProcessFailure { reason: "Downstream is full" }).await;
//                 },
//             }
//             self.time_to_next_event_counter = Some(Duration::from_secs_f64(self.process_time_distr.sample()));
//             // cx.schedule_event(MonotonicTime::EPOCH, Self::check_update_state, notif_meta).unwrap();
//         }
//     }

//     pub fn log<'a>(&'a mut self, time: MonotonicTime, details: NewVectorProcessLogType<T>) -> impl Future<Output = ()> + Send {
//         async move {
//             let log = NewVectorProcessLog {
//                 time: time.to_chrono_date_time(0).unwrap().to_string(),
//                 event_id: self.next_event_id,
//                 element_name: self.element_name.clone(),
//                 element_type: self.element_type.clone(),
//                 event: details,
//             };
//             self.next_event_id += 1;
//             self.log_emitter.send(log).await;
//         }
//     }
// }

pub struct NewVectorProcessLogger<T> {
    pub name: String,
    pub buffer: EventBuffer<NewVectorProcessLog<T>>,
}

impl Logger for NewVectorProcessLogger<f64> {
    type RecordType = NewVectorProcessLog<f64>;
    fn get_name(&self) -> &String {
        &self.name
    }
    fn get_buffer(self) -> EventBuffer<Self::RecordType> {
        self.buffer
    }
    fn new(name: String, capacity: usize) -> Self {
        NewVectorProcessLogger {
            name,
            buffer: EventBuffer::with_capacity(capacity),
        }
    }
}

impl Logger for NewVectorProcessLogger<Vector3> {
    type RecordType = NewVectorProcessLog<Vector3>;
    fn get_name(&self) -> &String {
        &self.name
    }
    fn get_buffer(self) -> EventBuffer<Self::RecordType> {
        self.buffer
    }
    fn new(name: String, capacity: usize) -> Self {
        NewVectorProcessLogger {
            name,
            buffer: EventBuffer::with_capacity(capacity),
        }
    }
}

#[derive(Debug, Clone)]
pub enum NewVectorProcessLogType<T> {
    ProcessStart { quantity: f64, vector: T },
    ProcessSuccess { quantity: f64, vector: T },
    ProcessFailure { reason: &'static str },
}

#[derive(Debug, Clone)]
pub struct NewVectorProcessLog<T> {
    pub time: String,
    pub event_id: u64,
    pub element_name: String,
    pub element_type: String,
    pub event: NewVectorProcessLogType<T>,
}

impl Serialize for NewVectorProcessLog<f64> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("NewVectorProcessLog", 6)?;
        state.serialize_field("time", &self.time)?;
        state.serialize_field("event_id", &self.event_id)?;
        state.serialize_field("element_name", &self.element_name)?;
        state.serialize_field("element_type", &self.element_type)?;
        let (event_type, total, reason): (&str, Option<f64>, Option<&str>) = match &self.event {
            NewVectorProcessLogType::ProcessStart { quantity, .. } => ("ProcessStart", Some(*quantity), None),
            NewVectorProcessLogType::ProcessSuccess { quantity, .. } => ("ProcessSuccess", Some(*quantity), None),
            NewVectorProcessLogType::ProcessFailure { reason, .. } => ("ProcessFailure", None, Some(*reason)),
        };
        state.serialize_field("event_type", &event_type)?;
        state.serialize_field("total", &total)?;
        state.serialize_field("reason", &reason)?;
        state.end()
    }
}

impl Serialize for NewVectorProcessLog<Vector3> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("NewVectorProcessLog", 6)?;
        state.serialize_field("time", &self.time)?;
        state.serialize_field("event_id", &self.event_id)?;
        state.serialize_field("element_name", &self.element_name)?;
        state.serialize_field("element_type", &self.element_type)?;
        let (event_type, total, x0, x1, x2, reason): (&str, Option<f64>, Option<f64>, Option<f64>, Option<f64>, Option<&str>) = match &self.event {
            NewVectorProcessLogType::ProcessStart { quantity, vector } => ("ProcessStart", Some(*quantity), Some(vector.values[0]), Some(vector.values[1]), Some(vector.values[2]), None),
            NewVectorProcessLogType::ProcessSuccess { quantity, vector } => ("ProcessSuccess", Some(*quantity), Some(vector.values[0]), Some(vector.values[1]), Some(vector.values[2]), None),
            NewVectorProcessLogType::ProcessFailure { reason, .. } => ("ProcessFailure", None, None, None, None, Some(reason)),
        };
        state.serialize_field("event_type", &event_type)?;
        state.serialize_field("total", &total)?;
        state.serialize_field("x0", &x0)?;
        state.serialize_field("x1", &x1)?;
        state.serialize_field("x2", &x2)?;
        state.serialize_field("reason", &reason)?;
        state.end()
    }
}
