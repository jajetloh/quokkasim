#![allow(clippy::manual_async_fn)]

use std::{error::Error, fs::create_dir_all, time::Duration};

use quokkasim::nexosim::Mailbox;
use quokkasim::prelude::*;
use quokkasim::define_model_enums;
use serde::{Serialize, ser::SerializeStruct};

/*
 * A representation of Iron Ore, primarily through iron content (Fe) and other elements.
 * Other relevant properties (e.g. mass comprised of magnetite, hematite, limonite minerals) are
 * also tracked.
 * 
 * All of these quantities are masses and must be provided in the same units, in order for
 * addition to be linear. Proportionate quantities (e.g. Mass % of ore comprised by Fe) can be calculated
 * using linear quantities (e.g. Fe % = Fe / (Fe + Other Elements) * 100). 
 */

#[derive(Debug, Clone)]
struct IronOre {
    fe: f64,
    other_elements: f64,
    magnetite: f64,
    hematite: f64,
    limonite: f64,
}

impl Default for IronOre {
    fn default() -> Self {
        IronOre {
            fe: 0.0,
            other_elements: 0.0,
            magnetite: 0.0,
            hematite: 0.0,
            limonite: 0.0,
        }
    }
}

impl ResourceAdd<IronOre> for IronOre {
    fn add(&mut self, other: Self) {
        self.fe += other.fe;
        self.other_elements += other.other_elements;
        self.magnetite += other.magnetite;
        self.hematite += other.hematite;
        self.limonite += other.limonite;
    }
}

impl ResourceRemove<f64, IronOre> for IronOre {
    fn remove(&mut self, quantity: f64) -> Self {
        let proportion_removed = quantity / self.total();
        let proportion_remaining = 1.0 - proportion_removed;
        
        let removed = IronOre {
            fe: self.fe * proportion_removed,
            other_elements: self.other_elements * proportion_removed,
            magnetite: self.magnetite * proportion_removed,
            hematite: self.hematite * proportion_removed,
            limonite: self.limonite * proportion_removed,
        };

        self.fe *= proportion_remaining;
        self.other_elements *= proportion_remaining;
        self.magnetite *= proportion_remaining;
        self.hematite *= proportion_remaining;
        self.limonite *= proportion_remaining;

        removed
    }
}

impl ResourceTotal<f64> for IronOre {
    fn total(&self) -> f64 {
        self.fe + self.other_elements
    }
}

impl ResourceMultiply<f64> for IronOre {
    fn multiply(&mut self, factor: f64) {
        self.fe *= factor;
        self.other_elements *= factor;
        self.magnetite *= factor;
        self.hematite *= factor;
        self.limonite *= factor;
    }
}

struct IronOreProcessLog {
    time: String,
    event_id: EventId,
    source_event_id: EventId,
    element_name: String,
    element_type: String,
    event: VectorProcessLogType<IronOre>,
}
impl Serialize for IronOreProcessLog {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("IronOreProcessLog", 16)?;
        state.serialize_field("time", &self.time)?;
        state.serialize_field("event_id", &self.event_id)?;
        state.serialize_field("source_event_id", &self.source_event_id)?;
        state.serialize_field("element_name", &self.element_name)?;
        state.serialize_field("element_type", &self.element_type)?;

        let (event_type, total, fe, other_elements, fe_pc, magnetite, hematite, limonite, message) = match &self.event {
            VectorProcessLogType::ProcessStart { quantity, vector } => {
                ("ProcessStart", Some(quantity), Some(vector.fe), Some(vector.other_elements), Some(vector.fe / vector.total()), Some(vector.magnetite), Some(vector.hematite), Some(vector.limonite), None)
            },
            VectorProcessLogType::ProcessSuccess { quantity, vector } => {
                ("ProcessSuccess", Some(quantity), Some(vector.fe), Some(vector.other_elements), Some(vector.fe / vector.total()), Some(vector.magnetite), Some(vector.hematite), Some(vector.limonite), None)
            },
            VectorProcessLogType::ProcessFailure { reason, .. } => {
                ("ProcessFailure", None, None, None, None, None, None, None, Some(reason))
            },
            VectorProcessLogType::WithdrawRequest => {
                ("WithdrawRequest", None, None, None, None, None, None, None, None)
            }
            x => {
               panic!("Unhandled VectorProcessLogType {:?}", x);
            }
        };
        state.serialize_field("event_type", &event_type)?;
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

/*
 * Defines a way to convert a `VectorProcessLog<IronOre>` into a custom `IronOreProcessLog` using `.into()`.
 */
impl From<VectorProcessLog<IronOre>> for IronOreProcessLog {
    fn from(log: VectorProcessLog<IronOre>) -> Self {
        IronOreProcessLog {
            time: log.time,
            event_id: log.event_id,
            source_event_id: log.source_event_id,
            element_name: log.element_name,
            element_type: log.element_type,
            event: log.event,
        }
    }
}

struct IronOreStockLog {
    time: String,
    event_id: EventId,
    source_event_id: EventId,
    element_name: String,
    element_type: String,
    details: VectorStockLogType<IronOre>,
}

impl Serialize for IronOreStockLog {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("IronOreStockLog", 12)?;
        state.serialize_field("time", &self.time)?;
        state.serialize_field("event_id", &self.event_id)?;
        state.serialize_field("source_event_id", &self.source_event_id)?;
        state.serialize_field("element_name", &self.element_name)?;
        state.serialize_field("element_type", &self.element_type)?;

        let (log_type, total, fe, other_elements, fe_pc, magnetite, hematite, limonite) = match &self.details {
            VectorStockLogType::Add { quantity, vector } => {
                ("Add", Some(vector.total()), Some(vector.fe), Some(vector.other_elements), Some(vector.fe / vector.total()), Some(vector.magnetite), Some(vector.hematite), Some(vector.limonite))
            },
            VectorStockLogType::Remove { quantity, vector } => {
                ("Remove", Some(vector.total()), Some(vector.fe), Some(vector.other_elements), Some(vector.fe / vector.total()), Some(vector.magnetite), Some(vector.hematite), Some(vector.limonite))
            },
            VectorStockLogType::EmitChange => {
                ("StateChange", None, None, None, None, None, None, None)
            },
            _ => {
                unimplemented!()
            }
        };
        state.serialize_field("log_type", &log_type)?;
        state.serialize_field("total", &total)?;
        state.serialize_field("fe", &fe)?;
        state.serialize_field("other_elements", &other_elements)?;
        state.serialize_field("fe_%", &fe_pc)?;
        state.serialize_field("magnetite", &magnetite)?;
        state.serialize_field("hematite", &hematite)?;
        state.serialize_field("limonite", &limonite)?;
        state.end()
    }
}

impl From<VectorStockLog<IronOre>> for IronOreStockLog {
    fn from(log: VectorStockLog<IronOre>) -> Self {
        IronOreStockLog {
            time: log.time,
            event_id: log.event_id,
            source_event_id: log.source_event_id,
            element_name: log.element_name,
            element_type: log.element_type,
            details: log.details,
        }
    }
}

//
// Define logger types for the IronOre components
//
struct IronOreProcessLogger {
    name: String,
    buffer: EventQueue<IronOreProcessLog>,
}

impl Logger for IronOreProcessLogger {
    type RecordType = IronOreProcessLog;

    fn get_name(&self) -> &String {
        &self.name
    }

    fn get_buffer(self) -> EventQueue<Self::RecordType> {
        self.buffer
    }

    fn new(name: String) -> Self {
        IronOreProcessLogger {
            name,
            buffer: EventQueue::new(),
        }
    }
}

struct IronOreStockLogger {
    name: String,
    buffer: EventQueue<IronOreStockLog>,
}

impl Logger for IronOreStockLogger {
    type RecordType = IronOreStockLog;

    fn get_name(&self) -> &String {
        &self.name
    }

    fn get_buffer(self) -> EventQueue<Self::RecordType> {
        self.buffer
    }

    fn new(name: String) -> Self {
        IronOreStockLogger {
            name,
            buffer: EventQueue::new(),
        }
    }
}

//
// Define the component and logger enums using the updated macro
//
define_model_enums! {
    pub enum ComponentModel {
        IronOreProcess(VectorProcess<f64, IronOre, IronOre, IronOre>, Mailbox<VectorProcess<f64, IronOre, IronOre, IronOre>>),
        IronOreStock(VectorStock<IronOre>, Mailbox<VectorStock<IronOre>>)
    }
    pub enum ComponentModelAddress {}
    pub enum ComponentLogger {
        IronOreProcessLogger(IronOreProcessLogger),
        IronOreStockLogger(IronOreStockLogger)
    }
    pub enum ScheduledEventConfig {
    }
}

impl CustomComponentConnection for ComponentModel {
    fn connect_components(a: &mut Self, b: &mut Self, n: Option<usize>) -> Result<(), Box<dyn Error>> {
        match (a, b) {
            (ComponentModel::IronOreProcess(a, ad), ComponentModel::IronOreStock(b, bd)) => {
                b.state_emitter.connect(VectorProcess::update_state, ad.address());
                a.req_downstream.connect(VectorStock::get_state_async, bd.address());
                a.push_downstream.connect(VectorStock::add, bd.address());
                Ok(())
            },
            (ComponentModel::IronOreStock(a, ad), ComponentModel::IronOreProcess(b, bd)) => {
                a.state_emitter.connect(VectorProcess::update_state, bd.address());
                b.req_upstream.connect(VectorStock::get_state_async, ad.address());
                b.withdraw_upstream.connect(VectorStock::remove, ad.address());
                Ok(())
            },
            (a, b) => Err(format!("No component connection defined from {} to {}", a, b).into()),
        }
    }
}

impl CustomLoggerConnection for ComponentLogger { 
    type ComponentType = ComponentModel;
    fn connect_logger(a: &mut Self, b: &mut Self::ComponentType, n: Option<usize>) -> Result<(), Box<dyn Error>> {
        match (a, b, n) {
            /*
             * As we are using the `VectorProcess<T>` and `VectorStock<T>` components, logs returned are `VectorProcessLog<T>` and `VectorStockLog<T>`
             * respectively.
             * 
             * These can be cumbersome as they nest properties into a single JSON-like field, so we can instead choose to convert these log structs into
             * our own custom `IronOreProcessLog` and `IronOreStockLog` structs. We can then implement a custom serialisation for these structs
             * to output fields in a flat format.
             */
            (ComponentLogger::IronOreProcessLogger(a), ComponentModel::IronOreProcess(b, _), _) => {
                b.log_emitter.map_connect_sink(|c| <VectorProcessLog<IronOre>>::into(c.clone()), &a.buffer);
                Ok(())
            },
            (ComponentLogger::IronOreStockLogger(a), ComponentModel::IronOreStock(b, _), _) => {
                b.log_emitter.map_connect_sink(|c| <VectorStockLog<IronOre>>::into(c.clone()), &a.buffer);
                Ok(())
            },
            (a, b, _) => Err(format!("No logger connection defined from {} to {}", a, b).into()),
        }
    }
}

fn main() {

    let base_seed = 123456789;
    let mut df = DistributionFactory::new(base_seed);

    let mut stock1 = ComponentModel::IronOreStock(
        VectorStock::new()
            .with_name("MyStock1")
            .with_code("SP1") 
            .with_name("IronOreStock")
            .with_initial_resource(IronOre { fe: 60., other_elements: 40., magnetite: 10., hematite: 5., limonite: 15. })
            .with_low_capacity(10.)
            .with_max_capacity(100.),
        Mailbox::new(),
    );

    let mut process1 = ComponentModel::IronOreProcess(
        VectorProcess::new()
            .with_name("MyProcess1")
            .with_code("P1") 
            .with_name("IronOreProcess")
            .with_process_quantity_distr(df.create(DistributionConfig::Uniform { min: 2., max: 8. }).unwrap())
            .with_process_time_distr(Distribution::Constant(10.)),
        Mailbox::new(),
    );
    let process1_addr = process1.get_address();

    let mut stock2 = ComponentModel::IronOreStock(
        VectorStock::new()
            .with_name("MyStock2")
            .with_code("SP2") 
            .with_name("IronOreStock")
            .with_initial_resource(IronOre { fe: 3., other_elements: 2., magnetite: 0.5, hematite: 0.25, limonite: 0.75 })
            .with_low_capacity(10.)
            .with_max_capacity(100.),
        Mailbox::new(),
    );

    connect_components!(&mut stock1, &mut process1).unwrap();
    connect_components!(&mut process1, &mut stock2).unwrap();

    let mut process_logger = ComponentLogger::IronOreProcessLogger(IronOreProcessLogger::new("IronOreProcessLogger".into()));
    let mut stock_logger = ComponentLogger::IronOreStockLogger(IronOreStockLogger::new("IronOreStockLogger".into()));
    
    connect_logger!(&mut stock_logger, &mut stock1).unwrap();
    connect_logger!(&mut process_logger, &mut process1).unwrap();
    connect_logger!(&mut stock_logger, &mut stock2).unwrap();

    let mut sim_builder = SimInit::new();
    sim_builder = register_component!(sim_builder, stock1);
    sim_builder = register_component!(sim_builder, process1);
    sim_builder = register_component!(sim_builder, stock2);

    let mut simu = sim_builder.init(MonotonicTime::EPOCH).unwrap().0;

    simu.step_until(MonotonicTime::EPOCH + Duration::from_secs_f64(300.)).unwrap();

    let output_dir = "outputs/iron_ore";

    create_dir_all(output_dir).unwrap();
    process_logger.write_csv(output_dir).unwrap();
    stock_logger.write_csv(output_dir).unwrap();

}