use nexosim::{model::Model, ports::{EventBuffer, Output, Requestor}};
use serde::{ser::SerializeStruct, Serialize};
use tai_time::MonotonicTime;
use std::{fmt::Debug, time::Duration};

use crate::{core::{Distribution, NotificationMetadata}, new_core::Process, prelude::{SubtractParts, Vector3, VectorArithmetic}};
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
impl<T: VectorArithmetic + Clone + Debug + Send> NewVectorStock<T> {
    pub fn get_state_async(&mut self) -> impl Future<Output=NewVectorStockState> + Send {
        async move {
            self.get_state()
        }
    }

    pub fn get_state(&self) -> NewVectorStockState {
        if self.vector.total() <= self.low_capacity {
            NewVectorStockState::Empty {
                occupied: 0.0,
                empty: self.max_capacity,
            }
        } else if self.vector.total() < self.max_capacity {
            NewVectorStockState::Normal {
                occupied: self.vector.total(),
                empty: self.max_capacity - self.vector.total(),
            }
        } else {
            NewVectorStockState::Full {
                occupied: self.vector.total(),
                empty: 0.0,
            }
        }
    }

    pub fn add(
        &mut self,
        data: (T, NotificationMetadata),
        cx: &mut ::nexosim::model::Context<Self>
    ) -> impl Future<Output=()> where NewVectorStock<T>: Model {
        async move {
            self.prev_state = Some(self.get_state());
            let added = self.vector.add(&data.0);
            self.vector = added.clone();
        }
    }

    pub fn remove(
        &mut self,
        data: (f64, NotificationMetadata),
        cx: &mut ::nexosim::model::Context<Self>
    ) -> impl Future<Output=T> where NewVectorStock<T>: Model {
        async move {
            self.prev_state = Some(self.get_state());
            let SubtractParts { subtracted, remaining } = self.vector.subtract_parts(data.0.clone());
            self.vector = remaining;
            subtracted
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
}

impl Logger for NewVectorStockLogger<Vector3> {
    type RecordType = NewVectorStockLog<Vector3>;
    fn get_name(&self) -> &String {
        &self.name
    }
    fn get_buffer(self) -> EventBuffer<Self::RecordType> {
        self.buffer
    }
}

/**
 * Process
 */

pub struct NewVectorProcess<T: VectorArithmetic + Clone + Debug + Send + 'static> {
    pub element_name: String,
    pub element_type: String,
    pub req_upstream: Requestor<(), NewVectorStockState>,
    pub req_downstream: Requestor<(), NewVectorStockState>,
    pub withdraw_upstream: Requestor<(f64, NotificationMetadata), T>,
    pub push_downstream: Output<(T, NotificationMetadata)>,
    pub process_quantity_distr: Distribution,
    pub process_time_distr: Distribution,
    pub time_to_next_event_counter: Option<Duration>,
    next_event_id: u64,
    pub log_emitter: Output<NewVectorProcessLog<T>>,
}
impl<T: VectorArithmetic + Clone + Debug + Default + Send> Default for NewVectorProcess<T> {
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
        }
    }
}

impl<T: VectorArithmetic + Send + 'static + Clone + Debug> Model for NewVectorProcess<T> {}

impl Process<f64> for NewVectorProcess<f64> {}
impl Process<Vector3> for NewVectorProcess<Vector3> {}

impl<T: VectorArithmetic + Clone + Debug + Send> NewVectorProcess<T> where Self: Model {
    pub fn check_update_state<'a>(
        &'a mut self,
        notif_meta: NotificationMetadata,
        cx: &'a mut ::nexosim::model::Context<Self>,
    ) -> impl Future<Output = ()> + 'a {
        async move {
            let time = cx.time();
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
                },
                (Some(NewVectorStockState::Empty {..} ), _) => {
                    self.log(time, NewVectorProcessLogType::ProcessFailure { reason: "Upstream is empty" }).await;
                },
                (None, _) => {
                    self.log(time, NewVectorProcessLogType::ProcessFailure { reason: "Upstream is not connected" }).await;
                },
                (_, None) => {
                    self.log(time, NewVectorProcessLogType::ProcessFailure { reason: "Downstream is not connected" }).await;
                },
                (_, Some(NewVectorStockState::Full {..} )) => {
                    self.log(time, NewVectorProcessLogType::ProcessFailure { reason: "Downstream is full" }).await;
                },
            }
            self.time_to_next_event_counter = Some(Duration::from_secs_f64(self.process_time_distr.sample()));
        }
    }

    pub fn log<'a>(&'a mut self, time: MonotonicTime, details: NewVectorProcessLogType<T>) -> impl Future<Output = ()> + Send {
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
}

impl Logger for NewVectorProcessLogger<Vector3> {
    type RecordType = NewVectorProcessLog<Vector3>;
    fn get_name(&self) -> &String {
        &self.name
    }
    fn get_buffer(self) -> EventBuffer<Self::RecordType> {
        self.buffer
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
