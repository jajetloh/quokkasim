use std::fs::File;

use csv::WriterBuilder;
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

    // fn write_csv(self, dir: String) -> Result<(), Box<dyn std::error::Error>>
    //     where
    //         Self: Sized, {
    //     let file = File::create(format!("{}/{}.csv", dir, self.get_name()))?;
    //     let mut writer = WriterBuilder::new().has_headers(true).from_writer(file);
    //     self.get_buffer().into_reader().for_each(|log| {
    //         writer
    //             .serialize(log)
    //             .expect("Failed to write log record to CSV file");
    //     });
    //     writer.flush()?;
    //     Ok(())
    // }
}

#[derive(Clone, Debug)]
pub struct TruckingProcessLog {
    pub time: String,
    pub event_id: u64,
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
            _ => {
                unimplemented!()
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

// struct IronOreProcessLog {
//     time: String,
//     event_id: u64,
//     element_name: String,
//     element_type: String,
//     log_type: String,
//     truck_id: u32,
//     event: VectorProcessLogType<IronOre>,
// }
// impl Serialize for IronOreProcessLog {
//     fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
//     where
//         S: serde::Serializer,
//     {
//         let mut state = serializer.serialize_struct("IronOreProcessLog", 15)?;
//         state.serialize_field("time", &self.time)?;
//         state.serialize_field("event_id", &self.event_id)?;
//         state.serialize_field("element_name", &self.element_name)?;
//         state.serialize_field("element_type", &self.element_type)?;
//         state.serialize_field("log_type", &self.log_type)?;
//         state.serialize_field("truck_id", &self.truck_id)?;

//         let (event_type, total, fe, other_elements, fe_pc, magnetite, hematite, limonite, message) = match &self.event {
//             VectorProcessLogType::ProcessStart { quantity, vector } => {
//                 ("ProcessStart", Some(quantity), Some(vector.fe), Some(vector.other_elements), Some(vector.fe / vector.total()), Some(vector.magnetite), Some(vector.hematite), Some(vector.limonite), None)
//             },
//             VectorProcessLogType::ProcessSuccess { quantity, vector } => {
//                 ("ProcessSuccess", Some(quantity), Some(vector.fe), Some(vector.other_elements), Some(vector.fe / vector.total()), Some(vector.magnetite), Some(vector.hematite), Some(vector.limonite), None)
//             },
//             VectorProcessLogType::ProcessFailure { reason, .. } => {
//                 ("ProcessFailure", None, None, None, None, None, None, None, Some(reason))
//             },
//         };
//         state.serialize_field("event_type", &event_type)?;
//         state.serialize_field("total", &total)?;
//         state.serialize_field("fe", &fe)?;
//         state.serialize_field("other_elements", &other_elements)?;
//         state.serialize_field("fe_%", &fe_pc)?;
//         state.serialize_field("magnetite", &magnetite)?;
//         state.serialize_field("hematite", &hematite)?;
//         state.serialize_field("limonite", &limonite)?;
//         state.serialize_field("message", &message)?;
//         state.end()
//     }
// }
// impl From<VectorProcessLog<IronOre>> for IronOreProcessLog {
//     fn from(log: VectorProcessLog<IronOre>) -> Self {
//         IronOreProcessLog {
//             time: log.time,
//             event_id: log.event_id,
//             element_name: log.element_name,
//             element_type: log.element_type,
//             // TODO: treat log_type and truck_id properly
//             log_type: "LOGTYPE".into(),
//             truck_id: 0,
//             event: log.event,
//         }
//     }
// }

// struct IronOreProcessLogger {
//     name: String,
//     buffer: EventQueue<IronOreProcessLog>
// }

// impl Logger for IronOreProcessLogger {
//     type RecordType = IronOreProcessLog;
//     fn get_name(&self) -> &String {
//         &self.name
//     }
//     fn get_buffer(self) -> EventQueue<Self::RecordType> {
//         self.buffer
//     }
//     fn new(name: String) -> Self {
//         IronOreProcessLogger {
//             name,
//             buffer: EventQueue::new(),
//         }
//     }
// }

// struct IronOreStockLog {
//     time: String,
//     event_id: u64,
//     element_name: String,
//     element_type: String,
//     log_type: String,
//     truck_id: u32,
//     resource: IronOre,
// }
// impl Serialize for IronOreStockLog {
//     fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
//     where
//         S: serde::Serializer,
//     {
//         let mut state = serializer.serialize_struct("IronOreStockLog", 12)?;
//         state.serialize_field("time", &self.time)?; 
//         state.serialize_field("event_id", &self.event_id)?;
//         state.serialize_field("element_name", &self.element_name)?;
//         state.serialize_field("element_type", &self.element_type)?;
//         state.serialize_field("log_type", &self.log_type)?;
//         state.serialize_field("truck_id", &self.truck_id)?;
//         state.serialize_field("total", &self.resource.total())?;
//         state.serialize_field("fe", &self.resource.fe)?;
//         state.serialize_field("other_elements", &self.resource.other_elements)?;
//         state.serialize_field("fe_%", &(self.resource.fe / self.resource.total()))?;
//         state.serialize_field("magnetite", &self.resource.magnetite)?;
//         state.serialize_field("hematite", &self.resource.hematite)?;
//         state.serialize_field("limonite", &self.resource.limonite)?;
//         state.end()
//     }
// }
// impl From<VectorStockLog<IronOre>> for IronOreStockLog {
//     fn from(log: VectorStockLog<IronOre>) -> Self {
//         IronOreStockLog {
//             time: log.time,
//             event_id: log.event_id,
//             element_name: log.element_name,
//             element_type: log.element_type,
//             // TODO: treat log_type and truck_id properly
//             log_type: "LOGTYPE".into(),
//             truck_id: 0,
//             resource: log.vector
//         }
//     }
// }

// struct IronOreStockLogger {
//     name: String,
//     buffer: EventQueue<IronOreStockLog>
// }
// impl Logger for IronOreStockLogger {
//     type RecordType = IronOreStockLog;
//     fn get_name(&self) -> &String {
//         &self.name
//     }
//     fn get_buffer(self) -> EventQueue<Self::RecordType> {
//         self.buffer
//     }
//     fn new(name: String) -> Self {
//         IronOreStockLogger {
//             name,
//             buffer: EventQueue::new(),
//         }
//     }
// }
// pub struct TruckStockLogger<T> where T: Send {
//     pub name: String,
//     pub buffer: EventQueue<TruckStockLog<T>>,
// }

// impl<T> Logger for TruckStockLogger<T> where TruckStockLog<T>: Serialize, T: Send + 'static {
//     type RecordType = TruckStockLog<T>;
//     fn get_name(&self) -> &String {
//         &self.name
//     }
//     fn get_buffer(self) -> EventQueue<Self::RecordType> {
//         self.buffer
//     }
//     fn new(name: String) -> Self {
//         TruckStockLogger {
//             name,
//             buffer: EventQueue::new(),
//         }
//     }
// }

// #[derive(Debug, Clone)]
// pub struct TruckStockLog<T> {
//     pub time: String,
//     pub event_id: u64,
//     pub element_name: String,
//     pub element_type: String,
//     pub log_type: String,
//     pub state: TruckStockState,
//     pub sequence: SeqDeque<T>,
// }

// impl Serialize for TruckStockLog<String> {
//     fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
//         where
//             S: serde::Serializer {
//         let mut state = serializer.serialize_struct("TruckStockLog", 6)?;
//         state.serialize_field("time", &self.time)?;
//         state.serialize_field("event_id", &self.event_id)?;
//         state.serialize_field("element_name", &self.element_name)?;
//         state.serialize_field("element_type", &self.element_type)?;
//         state.serialize_field("log_type", &self.log_type)?;
//         state.serialize_field("state", &self.state.get_name())?;
//         state.serialize_field("sequence", &format!("{:?}", self.sequence.deque))?;
//         state.end()
//     }
// }