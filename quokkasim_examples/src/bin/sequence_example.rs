use quokkasim::prelude::*;
use quokkasim::define_model_enums;
use std::error::Error;
use std::fs::create_dir_all;
use std::time::Duration;

define_model_enums! {
    pub enum ComponentModel<'a> {}
    pub enum ComponentLogger<'a> {}
}


impl<'a> CustomComponentConnection for ComponentModel<'a> {
    fn connect_components(a: Self, b: Self) -> Result<(), Box<dyn Error>> {
        Err(format!("connect_components not implemented from {} to {}", a, b).into())
    }
}

impl<'a> CustomLoggerConnection<'a> for ComponentLogger<'a> {
    type ComponentType = ComponentModel<'a>;
    fn connect_logger(a: Self, b: Self::ComponentType) -> Result<(), Box<dyn Error>> {
        Err(format!("connect_logger not implemented from {} to {}", a, b).into())
    }
}

fn main() {
    let mut stock1: DiscreteStock<String> = DiscreteStock::<String>::new()
        .with_name("Stock1".into())
        .with_type("SequenceStockString".into())
        .with_initial_contents((0..10).map(|x| format!("Item_{:0>4}", x)).collect())
        .with_low_capacity(0)
        .with_max_capacity(10);
    let stock1_mbox: Mailbox<DiscreteStock<String>> = Mailbox::new();
    let mut stock1_addr = stock1_mbox.address();

    let mut process1: DiscreteProcess<Option<String>, (), Option<String>> = DiscreteProcess::new()
        .with_name("Process1".into())
        .with_type("SequenceProcess".into())
        .with_process_time_distr(Distribution::Constant(7.));
    let process1_mbox: Mailbox<DiscreteProcess<Option<String>, (), Option<String>>> = Mailbox::new();
    let mut process1_addr = process1_mbox.address();

    let mut stock2: DiscreteStock<String> = DiscreteStock::<String>::new()
        .with_name("Stock2".into())
        .with_type("SequenceStockString".into())
        .with_low_capacity(0)
        .with_max_capacity(10);
    let stock2_mbox: Mailbox<DiscreteStock<String>> = Mailbox::new();
    let mut stock2_addr = stock2_mbox.address();

    ComponentModel::connect_components(
        ComponentModel::SequenceStockString(&mut stock1, &mut stock1_addr),
        ComponentModel::SequenceProcessString(&mut process1, &mut process1_addr),
    ).unwrap();
    ComponentModel::connect_components(
        ComponentModel::SequenceProcessString(&mut process1, &mut process1_addr),
        ComponentModel::SequenceStockString(&mut stock2, &mut stock2_addr),
    ).unwrap();

    let mut process_logger: DiscreteProcessLogger<Option<String>> = DiscreteProcessLogger::new("ProcessLogger".into());
    let mut stock_logger: DiscreteStockLogger<String> = DiscreteStockLogger::new("StockLogger".into());

    ComponentLogger::connect_logger(
        ComponentLogger::SequenceProcessLoggerString(&mut process_logger),
        ComponentModel::SequenceProcessString(&mut process1, &mut process1_addr),
    ).unwrap();
    ComponentLogger::connect_logger(
        ComponentLogger::SequenceStockLoggerString(&mut stock_logger),
        ComponentModel::SequenceStockString(&mut stock1, &mut stock1_addr),
    ).unwrap();
    ComponentLogger::connect_logger(
        ComponentLogger::SequenceStockLoggerString(&mut stock_logger),
        ComponentModel::SequenceStockString(&mut stock2, &mut stock2_addr),
    ).unwrap();

    let sim_builder = SimInit::new()
        .add_model(stock1, stock1_mbox, "Stock1")
        .add_model(process1, process1_mbox, "Process1")
        .add_model(stock2, stock2_mbox, "Stock2");
    let mut simu = sim_builder.init(MonotonicTime::EPOCH).unwrap().0;
    simu.process_event(DiscreteProcess::<Option<String>, (), Option<String>>::update_state,
        NotificationMetadata { time: MonotonicTime::EPOCH, element_from: "Init".into(), message: "Start".into() }, &process1_addr).unwrap();
    
    simu.step_until(MonotonicTime::EPOCH + Duration::from_secs(60)).unwrap();

    create_dir_all("outputs/sequence_example").unwrap();
    process_logger.write_csv("outputs/sequence_example".into()).unwrap();
    stock_logger.write_csv("outputs/sequence_example".into()).unwrap();
}