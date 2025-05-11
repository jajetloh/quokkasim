use std::{error::Error, fs::{create_dir, create_dir_all}, ops::Sub, time::Duration};

use quokkasim::nexosim::Mailbox;
use quokkasim::prelude::*;
use quokkasim::define_model_enums;
use serde::{ser::SerializeStruct, Serialize};

/**
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

impl VectorArithmetic for IronOre {
    fn add(&self, other: &Self) -> Self {
        IronOre {
            fe: self.fe + other.fe,
            other_elements: self.other_elements + other.other_elements,
            magnetite: self.magnetite + other.magnetite,
            hematite: self.hematite + other.hematite,
            limonite: self.limonite + other.limonite,
        }
    }

    fn subtract_parts(&self, quantity: f64) -> SubtractParts<IronOre> {
        let proportion_removed = quantity / self.total();
        let proportion_remaining = 1.0 - proportion_removed;
        SubtractParts {
            remaining: IronOre {
                fe: self.fe * proportion_remaining,
                other_elements: self.other_elements * proportion_remaining,
                magnetite: self.magnetite * proportion_remaining,
                hematite: self.hematite * proportion_remaining,
                limonite: self.limonite * proportion_remaining,
            },
            subtracted: IronOre {
                fe: self.fe * proportion_removed,
                other_elements: self.other_elements * proportion_removed,
                magnetite: self.magnetite * proportion_removed,
                hematite: self.hematite * proportion_removed,
                limonite: self.limonite * proportion_removed,
            },
        }
    }

    // We use the Fe + Other Elements as the 'source of truth' for the total mass
    fn total(&self) -> f64 {
        self.fe + self.other_elements
    }
}

struct IronOreProcessLog {
    time: String,
    element_name: String,
    element_type: String,
    log_type: String,
    truck_id: u32,
    event: VectorProcessLogType<IronOre>,
}
impl Serialize for IronOreProcessLog {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("IronOreProcessLog", 12)?;
        state.serialize_field("time", &self.time)?;
        state.serialize_field("element_name", &self.element_name)?;
        state.serialize_field("element_type", &self.element_type)?;
        state.serialize_field("log_type", &self.log_type)?;
        state.serialize_field("truck_id", &self.truck_id)?;

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
        };
        state.serialize_field("event_type", event_type)?;
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
impl From<VectorProcessLog<IronOre>> for IronOreProcessLog {
    fn from(log: VectorProcessLog<IronOre>) -> Self {
        IronOreProcessLog {
            time: log.time,
            element_name: log.element_name,
            element_type: log.element_type,
            // TODO: treat log_type and truck_id properly
            log_type: "LOGTYPE".into(),
            truck_id: 0,
            event: log.event,
        }
    }
}

struct IronOreProcessLogger {
    name: String,
    buffer: EventBuffer<IronOreProcessLog>
}

impl Logger for IronOreProcessLogger {
    type RecordType = IronOreProcessLog;
    fn get_name(&self) -> &String {
        &self.name
    }
    fn get_buffer(self) -> EventBuffer<Self::RecordType> {
        self.buffer
    }
    fn new(name: String, buffer_size: usize) -> Self {
        IronOreProcessLogger {
            name,
            buffer: EventBuffer::with_capacity(buffer_size),
        }
    }
}

struct IronOreStockLog {
    time: String,
    element_name: String,
    element_type: String,
    log_type: String,
    truck_id: u32,
    resource: IronOre,
}
impl Serialize for IronOreStockLog {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("IronOreStockLog", 12)?;
        state.serialize_field("time", &self.time)?;
        state.serialize_field("element_name", &self.element_name)?;
        state.serialize_field("element_type", &self.element_type)?;
        state.serialize_field("log_type", &self.log_type)?;
        state.serialize_field("truck_id", &self.truck_id)?;
        state.serialize_field("total", &self.resource.total())?;
        state.serialize_field("fe", &self.resource.fe)?;
        state.serialize_field("other_elements", &self.resource.other_elements)?;
        state.serialize_field("fe_%", &(self.resource.fe / self.resource.total()))?;
        state.serialize_field("magnetite", &self.resource.magnetite)?;
        state.serialize_field("hematite", &self.resource.hematite)?;
        state.serialize_field("limonite", &self.resource.limonite)?;
        state.end()
    }
}
impl From<VectorStockLog<IronOre>> for IronOreStockLog {
    fn from(log: VectorStockLog<IronOre>) -> Self {
        IronOreStockLog {
            time: log.time,
            element_name: log.element_name,
            element_type: log.element_type,
            // TODO: treat log_type and truck_id properly
            log_type: "LOGTYPE".into(),
            truck_id: 0,
            resource: log.vector
        }
    }
}

struct IronOreStockLogger {
    name: String,
    buffer: EventBuffer<IronOreStockLog>
}
impl Logger for IronOreStockLogger {
    type RecordType = IronOreStockLog;
    fn get_name(&self) -> &String {
        &self.name
    }
    fn get_buffer(self) -> EventBuffer<Self::RecordType> {
        self.buffer
    }
    fn new(name: String, buffer_size: usize) -> Self {
        IronOreStockLogger {
            name,
            buffer: EventBuffer::with_capacity(buffer_size),
        }
    }
}

define_model_enums! {
    pub enum ComponentModel<'a> {
        IronOreProcess(&'a mut VectorProcess<IronOre, IronOre, f64>, &'a mut Address<VectorProcess<IronOre, IronOre, f64>>),
        IronOreStock(&'a mut VectorStock<IronOre>, &'a mut Address<VectorStock<IronOre>>),
    }
    pub enum ComponentLogger<'a> {
        IronOreProcessLogger(&'a mut IronOreProcessLogger),
        IronOreStockLogger(&'a mut IronOreStockLogger),
    }
}

impl<'a> CustomComponentConnection for ComponentModel<'a> {
    fn connect_components(a: Self, b: Self) -> Result<(), Box<dyn Error>> {
        match (a, b) {
            (ComponentModel::IronOreProcess(a, ad), ComponentModel::IronOreStock(b, bd)) => {
                b.state_emitter.connect(VectorProcess::<IronOre, IronOre, f64>::update_state, ad.clone());
                a.req_downstream.connect(VectorStock::<IronOre>::get_state_async, bd.clone());
                a.push_downstream.connect(VectorStock::<IronOre>::add, bd.clone());
                Ok(())
            },
            (ComponentModel::IronOreStock(a, ad), ComponentModel::IronOreProcess(b, bd)) => {
                a.state_emitter.connect(VectorProcess::<IronOre, IronOre, f64>::update_state, bd.clone());
                b.req_upstream.connect(VectorStock::<IronOre>::get_state_async, ad.clone());
                b.withdraw_upstream.connect(VectorStock::<IronOre>::remove, ad.clone());
                Ok(())
            },
            _ => Err("Invalid connection".into()),
        }
    }
}

impl<'a> CustomLoggerConnection<'a> for ComponentLogger<'a> {
    type ComponentType = ComponentModel<'a>;
    fn connect_logger(a: Self, b: Self::ComponentType) -> Result<(), Box<dyn Error>> {
        match (a, b) {
            (ComponentLogger::IronOreProcessLogger(a), ComponentModel::IronOreProcess(b, _)) => {
                b.log_emitter.map_connect_sink(|c| <VectorProcessLog<IronOre>>::into(c.clone()), &a.buffer);
                Ok(())
            },
            (ComponentLogger::IronOreStockLogger(a), ComponentModel::IronOreStock(b, _)) => {
                b.log_emitter.map_connect_sink(|c| <VectorStockLog<IronOre>>::into(c.clone()), &a.buffer);
                Ok(())
            },
            _ => Err("Invalid connection".into()),
        }
    }
}

fn main() {

    let mut stock1 = VectorStock::<IronOre>::default().with_name("Stock1".into()).with_type("IronOreStock".into());
    stock1.vector = IronOre { fe: 60., other_elements: 40., magnetite: 10., hematite: 5., limonite: 15. };
    stock1.low_capacity = 10.;
    stock1.max_capacity = 100.;
    let stock1_mbox: Mailbox<VectorStock<IronOre>> = Mailbox::new();
    let mut stock1_addr = stock1_mbox.address();
    
    let mut process1 = VectorProcess::<IronOre, IronOre, f64>::default().with_name("Process1".into()).with_type("IronOreProcess".into());
    process1.process_quantity_distr = Distribution::Constant(4.);
    process1.process_time_distr = Distribution::Constant(10.);
    let process1_mbox: Mailbox<VectorProcess<IronOre, IronOre, f64>> = Mailbox::new();
    let mut process1_addr: Address<VectorProcess<IronOre, IronOre, f64>> = process1_mbox.address();

    let mut stock2 = VectorStock::<IronOre>::default().with_name("Stock2".into()).with_type("IronOreStock".into());
    stock2.vector = IronOre { fe: 3., other_elements: 2., magnetite: 0.5, hematite: 0.25, limonite: 0.75 };
    stock2.low_capacity = 10.;
    stock2.max_capacity = 100.;
    let stock2_mbox: Mailbox<VectorStock<IronOre>> = Mailbox::new();
    let mut stock2_addr = stock2_mbox.address();

    ComponentModel::connect_components(
        ComponentModel::IronOreStock(&mut stock1, &mut stock1_addr),
        ComponentModel::IronOreProcess(&mut process1, &mut process1_addr),
    ).unwrap();
    ComponentModel::connect_components(
        ComponentModel::IronOreProcess(&mut process1, &mut process1_addr),
        ComponentModel::IronOreStock(&mut stock2, &mut stock2_addr),
    ).unwrap();

    let mut process_logger = IronOreProcessLogger::new("IronOreProcessLogger".into(), 100_000);
    let mut stock_logger = IronOreStockLogger::new("IronOreStockLogger".into(), 100_000);

    ComponentLogger::connect_logger(
        ComponentLogger::IronOreProcessLogger(&mut process_logger),
        ComponentModel::IronOreProcess(&mut process1, &mut process1_addr),
    ).unwrap();
    ComponentLogger::connect_logger(
        ComponentLogger::IronOreStockLogger(&mut stock_logger),
        ComponentModel::IronOreStock(&mut stock1, &mut stock1_addr),
    ).unwrap();
    ComponentLogger::connect_logger(
        ComponentLogger::IronOreStockLogger(&mut stock_logger),
        ComponentModel::IronOreStock(&mut stock2, &mut stock2_addr),
    ).unwrap();

    let sim_builder = SimInit::new()
        .add_model(stock1, stock1_mbox, "Stock1")
        .add_model(process1, process1_mbox, "Process1")
        .add_model(stock2, stock2_mbox, "Stock2");
    let mut simu = sim_builder.init(MonotonicTime::EPOCH).unwrap().0;
    simu.process_event(
        VectorProcess::<IronOre, IronOre, f64>::update_state,
        NotificationMetadata {
            time: MonotonicTime::EPOCH,
            element_from: "Process1".into(),
            message: "Start".into(),
        }, &process1_addr
    ).unwrap();
    simu.step_until(MonotonicTime::EPOCH + Duration::from_secs_f64(300.)).unwrap();

    create_dir_all("outputs/trucking_2").unwrap();
    process_logger.write_csv("outputs/trucking_2".into()).unwrap();
    stock_logger.write_csv("outputs/trucking_2".into()).unwrap();

}