use std::error::Error;

use nexosim::model::Model;
use quokkasim::{components::new_vector::{NewVectorProcess, NewVectorStock}, core::Distribution, define_model_enums, new_core::*};

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

    fn subtract(&self, other: &Self) -> Self {
        Ore {
            cu: self.cu - other.cu,
            s: self.s - other.s,
            other: self.other - other.other,
        }
    }

    fn multiply(&self, scalar: f64) -> Self {
        Ore {
            cu: self.cu * scalar,
            s: self.s * scalar,
            other: self.other * scalar,
        }
    }

    fn divide(&self, scalar: f64) -> Self {
        Ore {
            cu: self.cu / scalar,
            s: self.s / scalar,
            other: self.other / scalar,
        }
    }

    fn total(&self) -> f64 {
        self.cu + self.s + self.other
    }
}

struct MyCustomStock {}

impl Model for MyCustomStock {}

struct MyCustomProcess {}

impl Model for MyCustomProcess {}

define_model_enums! {
    pub enum ComponentModel {
        MyCustomStock(MyCustomStock),
        MyCustomProcess(MyCustomProcess),
    }
    pub enum ComponentLogger {
    }
}

impl CustomComponentConnection for ComponentModel {
    fn connect_components(a: Self, b: Self) -> Result<(), Box<dyn Error>> {
        match (a, b) {
            (ComponentModel::MyCustomStock(_), ComponentModel::MyCustomStock(_)) => Ok(()),
            (ComponentModel::MyCustomProcess(_), ComponentModel::MyCustomStock(_)) => Ok(()),
            (ComponentModel::NewVectorStockF64(_), ComponentModel::MyCustomStock(_)) => Ok(()),
            _ => Err("Invalid connection".into()),
        }
    }
}

impl CustomLoggerConnection for ComponentLogger {
    type ComponentType = ComponentModel;
    fn connect_logger(a: Self, b: Self::ComponentType) -> Result<(), Box<dyn Error>> {
        match (a, b) {
            _ => Err("Invalid connection".into()),
        }
    }
}

fn main() {

    let stock1 = NewVectorStock::<Ore> {
        vector: Ore { cu: 1.0, s: 2.0, other: 3.0 },
        element_name: "Stock1".into(),
        element_type: "NewVectorStock".into(),
        low_capacity: 1.,
        max_capacity: 100.,
    };

    let process1: NewVectorProcess::<Ore> = NewVectorProcess {
        vector: Ore { cu: 1.0, s: 2.0, other: 3.0 },
        process_quantity_distr: Distribution::Constant(12.5),
        process_time_distr: Distribution::Constant(2.4),
    };

    println!("Hello, world!");
}