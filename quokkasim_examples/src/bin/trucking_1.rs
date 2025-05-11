use std::{error::Error, fs::create_dir_all, time::Duration};

use quokkasim::nexosim::Mailbox;
use quokkasim::prelude::*;
use quokkasim::define_model_enums;

define_model_enums! {
    pub enum ComponentModel<'a> {
    }
    pub enum ComponentLogger<'a> {
    }
}

impl<'a> CustomComponentConnection for ComponentModel<'a> {
    fn connect_components(a: Self, b: Self) -> Result<(), Box<dyn Error>> {
        match (a, b) {
            _ => Err("Invalid connection".into()),
        }
    }
}

impl<'a> CustomLoggerConnection<'a> for ComponentLogger<'a> {
    type ComponentType = ComponentModel<'a>;
    fn connect_logger(a: Self, b: Self::ComponentType) -> Result<(), Box<dyn Error>> {
        match (a, b) {
            _ => Err("Invalid connection".into()),
        }
    }
}

fn main() {

    let mut stock1 = VectorStock::<f64>::default().with_name("Stock1".into()).with_type("IronOreStock".into());
    stock1.vector = 100.;
    let stock1_mbox: Mailbox<VectorStock<f64>> = Mailbox::new();
    let mut stock1_addr = stock1_mbox.address();
    
    let mut process1 = VectorProcess::<f64, f64, f64>::default().with_name("Process1".into()).with_type("IronOreProcess".into());
    process1.process_quantity_distr = Distribution::Constant(4.);
    process1.process_time_distr = Distribution::Constant(10.);
    let process1_mbox: Mailbox<VectorProcess<f64, f64, f64>> = Mailbox::new();
    let mut process1_addr: Address<VectorProcess<f64, f64, f64>> = process1_mbox.address();

    let mut stock2 = VectorStock::<f64>::default().with_name("Stock2".into()).with_type("IronOreStock".into());
    stock2.vector = 5.;
    stock2.low_capacity = 10.;
    stock2.max_capacity = 100.;
    let stock2_mbox: Mailbox<VectorStock<f64>> = Mailbox::new();
    let mut stock2_addr = stock2_mbox.address();

    ComponentModel::connect_components(
        ComponentModel::VectorStockF64(&mut stock1, &mut stock1_addr),
        ComponentModel::VectorProcessF64(&mut process1, &mut process1_addr),
    ).unwrap();
    ComponentModel::connect_components(
        ComponentModel::VectorProcessF64(&mut process1, &mut process1_addr),
        ComponentModel::VectorStockF64(&mut stock2, &mut stock2_addr),
    ).unwrap();

    let mut process_logger = VectorProcessLogger::<f64>::new("IronOreProcessLogger".into(), 100_000);
    let mut stock_logger = VectorStockLogger::<f64>::new("IronOreStockLogger".into(), 100_000);

    ComponentLogger::connect_logger(
        ComponentLogger::VectorProcessLoggerF64(&mut process_logger),
        ComponentModel::VectorProcessF64(&mut process1, &mut process1_addr),
    ).unwrap();
    ComponentLogger::connect_logger(
        ComponentLogger::VectorStockLoggerF64(&mut stock_logger),
        ComponentModel::VectorStockF64(&mut stock1, &mut stock1_addr),
    ).unwrap();
    ComponentLogger::connect_logger(
        ComponentLogger::VectorStockLoggerF64(&mut stock_logger),
        ComponentModel::VectorStockF64(&mut stock2, &mut stock2_addr),
    ).unwrap();

    let sim_builder = SimInit::new()
        .add_model(stock1, stock1_mbox, "Stock1")
        .add_model(process1, process1_mbox, "Process1")
        .add_model(stock2, stock2_mbox, "Stock2");
    let mut simu = sim_builder.init(MonotonicTime::EPOCH).unwrap().0;
    simu.process_event(
        VectorProcess::<f64, f64, f64>::update_state,
        NotificationMetadata {
            time: MonotonicTime::EPOCH,
            element_from: "Process1".into(),
            message: "Start".into(),
        }, &process1_addr
    ).unwrap();
    simu.step_until(MonotonicTime::EPOCH + Duration::from_secs_f64(300.)).unwrap();

    create_dir_all("outputs/trucking_1").unwrap();
    process_logger.write_csv("outputs/trucking_1".into()).unwrap();
    stock_logger.write_csv("outputs/trucking_1".into()).unwrap();

}