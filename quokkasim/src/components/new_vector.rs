use nexosim::{model::Model, ports::{EventBuffer, Output}};
use serde::{ser::SerializeStruct, Serialize};
use std::fmt::Debug;

use crate::{core::Distribution, prelude::{Vector3, VectorArithmetic}};
use crate::new_core::Logger;

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

pub struct NewVectorStock<T: VectorArithmetic + Clone + Debug> {
    pub element_name: String,
    pub element_type: String,
    pub vector: T,
    // pub log_emitter: Output<>,
    pub low_capacity: f64,
    pub max_capacity: f64
}
impl<T: VectorArithmetic + Clone + Debug + Default> Default for NewVectorStock<T> {
    fn default() -> Self {
        NewVectorStock {
            element_name: String::new(),
            element_type: String::new(),
            vector: Default::default(),
            low_capacity: 0.0,
            max_capacity: 0.0,
        }
    }
}
impl<T: VectorArithmetic + Clone + Debug> NewVectorStock<T> {
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
}

pub struct NewVectorStockLogger<T> {
    pub name: String,
    pub buffer: EventBuffer<NewVectorStockLog<T>>,
}

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


// impl<T: VectorArithmetic + Send + 'static + Clone + Debug> Model for NewVectorStock<T> {}

// pub struct NewVectorStockLog<T: Serialize> {
//     pub time: String,
//     pub event_id: String,
//     pub element_name: String,
//     pub element_type: String,
//     pub log_type: String,
//     pub state: NewVectorStockState,
//     pub vector: T
// }







pub struct NewVectorProcess<T: VectorArithmetic + Clone + Debug> {
    pub vector: T,
    pub process_quantity_distr: Distribution,
    pub process_time_distr: Distribution,
}
impl<T: VectorArithmetic + Clone + Debug + Default> Default for NewVectorProcess<T> {
    fn default() -> Self {
        NewVectorProcess {
            vector: Default::default(),
            process_quantity_distr: Distribution::default(),
            process_time_distr: Distribution::default(),
        }
    }
}

impl<T: VectorArithmetic + Send + 'static + Clone + Debug> Model for NewVectorProcess<T> {}

pub struct NewVectorProcessLogger<T> {
    pub name: String,
    pub buffer: EventBuffer<NewVectorProcessLog<T>>,
}
// impl Logger for NewVectorStockLogger<f64> {
//     type RecordType = NewVectorStockLog<f64>;
//     fn get_name(&self) -> &String {
//         &self.name
//     }
//     fn get_buffer(self) -> EventBuffer<Self::RecordType> {
//         self.buffer
//     }
// }

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

pub struct NewVectorProcessLog<T> {
    pub time: String,
    pub event_id: String,
    pub element_name: String,
    pub element_type: String,
    pub log_type: String,
    pub state: NewVectorStockState,
    pub vector: T,
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
        state.serialize_field("log_type", &self.log_type)?;
        state.serialize_field("state", &self.state.get_name())?;
        state.serialize_field("value", &self.vector)?;
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
        state.serialize_field("log_type", &self.log_type)?;
        state.serialize_field("state", &self.state.get_name())?;
        state.serialize_field("x0", &self.vector.values[0])?;
        state.serialize_field("x1", &self.vector.values[1])?;
        state.serialize_field("x2", &self.vector.values[2])?;
        state.end()
    }
}
