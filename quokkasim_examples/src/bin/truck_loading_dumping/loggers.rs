#![allow(clippy::manual_async_fn)]

use quokkasim::prelude::*;
use serde::{ser::SerializeStruct, Serialize};

use crate::{iron_ore::IronOre, truck::Truck};

pub struct TruckingProcessLogger {
    pub name: String,
    pub buffer: EventQueue<TruckingProcessLog>,
}

impl Logger for TruckingProcessLogger {
    type RecordType = TruckingProcessLog;

    fn get_name(&self) -> &String {
        &self.name
    }

    fn get_buffer(self) -> EventQueue<Self::RecordType> {
        self.buffer
    }

    fn new(name: String) -> Self {
        TruckingProcessLogger {
            name,
            buffer: EventQueue::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct TruckingProcessLog {
    pub time: String,
    pub event_id: EventId,
    pub source_event_id: EventId,
    pub element_name: String,
    pub element_type: String,
    pub event: TruckingProcessLogType,
}

#[derive(Clone, Debug)]
pub enum TruckingProcessLogType {
    LoadingStart { truck_id: String, quantity: f64, ore: IronOre },
    LoadingSuccess { truck_id: String, quantity: f64, ore: IronOre },
    LoadingFailure { reason: &'static str },
    DumpingStart { truck_id: String, quantity: f64, ore: IronOre },
    DumpingSuccess { truck_id: String, quantity: f64, ore: IronOre },
    DumpingFailure { reason: &'static str },
    TruckMovementStart { truck_id: String },
    TruckMovementSuccess { truck_id: String },
    TruckMovementFailure { reason: &'static str },
    WithdrawRequest,
    PushRequest,
}

impl Serialize for TruckingProcessLog {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("TruckingProcessLog", 15)?;
        state.serialize_field("time", &self.time)?;
        state.serialize_field("event_id", &self.event_id)?;
        state.serialize_field("element_name", &self.element_name)?;
        state.serialize_field("element_type", &self.element_type)?;

        let (event_type, truck_id, total, fe, other_elements, fe_pc, magnetite, hematite, limonite, message) = match &self.event {
            TruckingProcessLogType::LoadingStart { truck_id, quantity, ore: vector } => {
                ("ProcessStart", Some(truck_id.clone()), Some(quantity), Some(vector.fe), Some(vector.other_elements), Some(vector.fe / vector.total()), Some(vector.magnetite), Some(vector.hematite), Some(vector.limonite), None)
            },
            TruckingProcessLogType::LoadingSuccess { truck_id, quantity, ore: vector } => {
                ("ProcessSuccess", Some(truck_id.clone()), Some(quantity), Some(vector.fe), Some(vector.other_elements), Some(vector.fe / vector.total()), Some(vector.magnetite), Some(vector.hematite), Some(vector.limonite), None)
            },
            TruckingProcessLogType::LoadingFailure { reason, .. } => {
                ("ProcessFailure", None, None, None, None, None, None, None, None, Some(reason))
            },
            TruckingProcessLogType::DumpingStart { truck_id, quantity, ore: vector } => {
                ("ProcessStart", Some(truck_id.clone()), Some(quantity), Some(vector.fe), Some(vector.other_elements), Some(vector.fe / vector.total()), Some(vector.magnetite), Some(vector.hematite), Some(vector.limonite), None)
            },
            TruckingProcessLogType::DumpingSuccess { truck_id, quantity, ore: vector } => {
                ("ProcessSuccess", Some(truck_id.clone()), Some(quantity), Some(vector.fe), Some(vector.other_elements), Some(vector.fe / vector.total()), Some(vector.magnetite), Some(vector.hematite), Some(vector.limonite), None)
            },
            TruckingProcessLogType::DumpingFailure { reason, .. } => {
                ("ProcessFailure", None, None, None, None, None, None, None, None, Some(reason))
            },
            TruckingProcessLogType::TruckMovementStart { truck_id } => {
                ("TruckMovementStart", Some(truck_id.clone()), None, None, None, None, None, None, None, None)
            },
            TruckingProcessLogType::TruckMovementSuccess { truck_id } => {
                ("TruckMovementSuccess", Some(truck_id.clone()), None, None, None, None, None, None, None, None)
            },
            TruckingProcessLogType::TruckMovementFailure { reason, .. } => {
                ("TruckMovementFailure", None, None, None, None, None, None, None, None, Some(reason))
            },
            TruckingProcessLogType::WithdrawRequest => {
                ("WithdrawRequest", None, None, None, None, None, None, None, None, None)
            },
            _ => {
                panic!("Serialisation not yet implemented for: {:?}", self.event);
            }
        };
        state.serialize_field("event_type", &event_type)?;
        state.serialize_field("truck_id", &truck_id)?;
        state.serialize_field("total", &total)?;
        state.serialize_field("fe", &fe)?;
        state.serialize_field("other_elements", &other_elements)?;
        state.serialize_field("fe_%", &fe_pc)?;
        state.serialize_field("magnetite", &magnetite)?;
        state.serialize_field("hematite", &hematite)?;
        state.serialize_field("limonite", &limonite)?;
        state.serialize_field("message", &message)?;
        state.end()
    }
}

pub struct TruckingStockLog {
    pub time: String,
    pub event_id: u64,
    pub element_name: String,
    pub element_type: String,
    pub log_type: String,
    pub state: DiscreteStockState,
    pub resource: ItemDeque<Truck>,
}

impl Serialize for TruckingStockLog {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("TruckingStockLog", 7)?;
        state.serialize_field("time", &self.time)?;
        state.serialize_field("event_id", &self.event_id)?;
        state.serialize_field("element_name", &self.element_name)?;
        state.serialize_field("element_type", &self.element_type)?;
        state.serialize_field("log_type", &self.log_type)?;
        state.serialize_field("state", match self.state {
            DiscreteStockState::Empty { .. } => "Empty",
            DiscreteStockState::Normal { .. } => "Normal",
            DiscreteStockState::Full { .. } => "Full",
        })?;
        state.serialize_field("num_trucks", &self.resource.total())?;
        state.serialize_field("trucks", &self.resource.iter().map(|truck| truck.truck_id.clone()).collect::<Vec<String>>().join("|"))?;
        state.end()
    }
}