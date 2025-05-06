use std::{error::Error, ops::Sub, time::Duration};

use nexosim::{model::Model, time::MonotonicTime};
use quokkasim::{components::new_vector::{NewVectorProcess, NewVectorStock}, core::{Distribution, Mailbox, NotificationMetadata, SimInit}, define_model_enums, new_core::*};

#[derive(Debug, Clone)]
struct Ore {
    cu: f64,
    s: f64,
    other: f64,
}

impl VectorArithmetic for Ore {
    fn add(&self, other: &Self) -> Self {
        Ore {
            cu: self.cu + other.cu,
            s: self.s + other.s,
            other: self.other + other.other,
        }
    }

    fn subtract_parts(&self, quantity: f64) -> SubtractParts<Ore> {
        let proportion_removed = quantity / self.total();
        let proportion_remaining = 1.0 - proportion_removed;
        SubtractParts {
            remaining: Ore {
                cu: self.cu * proportion_remaining,
                s: self.s * proportion_remaining,
                other: self.other * proportion_remaining,
            },
            subtracted: Ore {
                cu: self.cu * proportion_removed,
                s: self.s * proportion_removed,
                other: self.other * proportion_removed,
            },
        }
    }

    fn total(&self) -> f64 {
        self.cu + self.s + self.other
    }
}



define_model_enums! {
    pub enum ComponentModel {
        // MyCustomStock(MyCustomStock),
        // MyCustomProcess(MyCustomProcess),
    }
    pub enum ComponentLogger {
    }
}

impl<'a> CustomComponentConnection for ComponentModel<'a> {
    fn connect_components(a: Self, b: Self) -> Result<(), Box<dyn Error>> {
        match (a, b) {
            // (ComponentModel::MyCustomStock(_), ComponentModel::MyCustomStock(_)) => Ok(()),
            // (ComponentModel::MyCustomProcess(_), ComponentModel::MyCustomStock(_)) => Ok(()),
            // (ComponentModel::NewVectorStockF64(_), ComponentModel::MyCustomStock(_)) => Ok(()),
            _ => Err("Invalid connection".into()),
        }
    }
}

impl<'a> CustomLoggerConnection<'a> for ComponentLogger {
    type ComponentType = ComponentModel<'a>;
    fn connect_logger(a: Self, b: Self::ComponentType) -> Result<(), Box<dyn Error>> {
        match (a, b) {
            _ => Err("Invalid connection".into()),
        }
    }
}

fn main() {

    let mut stock1 = NewVectorStock::<f64>::default();
    stock1.vector = 100.;
    stock1.low_capacity = 10.;
    stock1.max_capacity = 100.;
    let stock1_mbox: Mailbox<NewVectorStock<f64>> = Mailbox::new();
    let mut stock1_addr = stock1_mbox.address();
    
    let mut process1 = NewVectorProcess::<f64>::default();
    process1.process_quantity_distr = Distribution::Constant(4.);
    process1.process_time_distr = Distribution::Constant(10.);
    let process1_mbox: Mailbox<NewVectorProcess<f64>> = Mailbox::new();
    let mut process1_addr = process1_mbox.address();

    let mut stock2 = NewVectorStock::<f64>::default();
    stock2.vector = 5.;
    stock2.low_capacity = 10.;
    stock2.max_capacity = 100.;
    let stock2_mbox: Mailbox<NewVectorStock<f64>> = Mailbox::new();
    let mut stock2_addr = stock2_mbox.address();

    ComponentModel::connect_components(
        ComponentModel::NewVectorStockF64(&mut stock1, &mut stock1_addr),
        ComponentModel::NewVectorProcessF64(&mut process1, &mut process1_addr),
    ).unwrap();
    ComponentModel::connect_components(
        ComponentModel::NewVectorProcessF64(&mut process1, &mut process1_addr),
        ComponentModel::NewVectorStockF64(&mut stock2, &mut stock2_addr),
    ).unwrap();

    let sim_builder = SimInit::new()
        .add_model(stock1, stock1_mbox, "Stock1")
        .add_model(process1, process1_mbox, "Process1")
        .add_model(stock2, stock2_mbox, "Stock2");
    let mut simu = sim_builder.init(MonotonicTime::EPOCH).unwrap().0;
    simu.process_event(
        NewVectorProcess::check_update_state,
        NotificationMetadata {
            time: MonotonicTime::EPOCH,
            element_from: "Process1".into(),
            message: "Start".into(),
        }, &process1_addr
    ).unwrap();
    simu.step_until(MonotonicTime::EPOCH + Duration::from_secs_f64(3600.)).unwrap();

    println!("Hello, world!");
}