#![allow(clippy::manual_async_fn)]

use std::{error::Error, fs::create_dir_all, time::Duration};
use quokkasim::{define_model_enums, prelude::*};
use serde::Serialize;

#[derive(Debug, Clone, Default, Serialize)]
struct ProtoCar {
    id: usize,
}

#[derive(Default)]
struct ProtoCarGenerator {
    next_id: usize
}


impl ItemFactory<ProtoCar> for ProtoCarGenerator {
    fn create_item(&mut self) -> ProtoCar {
        let result = ProtoCar { id: self.next_id };
        self.next_id += 1;
        result
    }
}

define_model_enums! {
    pub enum ComponentModel {
        ProtoCarProcess(DiscreteProcess<(), Option<ProtoCar>, ProtoCar, ProtoCar>, Mailbox<DiscreteProcess<(), Option<ProtoCar>, ProtoCar, ProtoCar>>),
        ProtoCarParallelProcess(DiscreteParallelProcess<(), Option<ProtoCar>, ProtoCar, ProtoCar>, Mailbox<DiscreteParallelProcess<(), Option<ProtoCar>, ProtoCar, ProtoCar>>),
        ProtoCarStock(DiscreteStock<ProtoCar>, Mailbox<DiscreteStock<ProtoCar>>),
        ProtoCarSource(DiscreteSource<ProtoCar, ProtoCar, ProtoCarGenerator>, Mailbox<DiscreteSource<ProtoCar, ProtoCar, ProtoCarGenerator>>),
        ProtoCarSink(DiscreteSink<(), Option<ProtoCar>, ProtoCar>, Mailbox<DiscreteSink<(), Option<ProtoCar>, ProtoCar>>),
    }
    pub enum ComponentModelAddress {}
    pub enum ComponentLogger {
        ProtoCarProcessLogger(DiscreteProcessLogger<ProtoCar>),
        ProtoCarStockLogger(DiscreteStockLogger<ProtoCar>),
    }
    pub enum ScheduledEvent {}
}

impl CustomComponentConnection for ComponentModel {
    fn connect_components(a: &mut Self, b: &mut Self, n: Option<usize>) -> Result<(), Box<dyn Error>> {
        match (a, b) {
            (ComponentModel::ProtoCarSource(source, source_mbox), ComponentModel::ProtoCarStock(stock, stock_mbox)) => {
                source.req_downstream.connect(DiscreteStock::get_state_async, stock_mbox.address());
                source.push_downstream.connect(DiscreteStock::add, stock_mbox.address());
                stock.state_emitter.connect(DiscreteSource::update_state, source_mbox.address());
                Ok(())
            },
            (ComponentModel::ProtoCarStock(stock, stock_mbox), ComponentModel::ProtoCarProcess(process, process_mbox)) => {
                process.req_upstream.connect(DiscreteStock::get_state_async, stock_mbox.address());
                process.withdraw_upstream.connect(DiscreteStock::remove, stock_mbox.address());
                stock.state_emitter.connect(DiscreteProcess::update_state, process_mbox.address());
                Ok(())
            },
            (ComponentModel::ProtoCarProcess(process, process_mbox), ComponentModel::ProtoCarStock(stock, stock_mbox)) => {
                process.req_downstream.connect(DiscreteStock::get_state_async, stock_mbox.address());
                process.push_downstream.connect(DiscreteStock::add, stock_mbox.address());
                stock.state_emitter.connect(DiscreteProcess::update_state, process_mbox.address());
                Ok(())
            },
            (ComponentModel::ProtoCarParallelProcess(process, process_mbox), ComponentModel::ProtoCarStock(stock, stock_mbox)) => {
                process.req_downstream.connect(DiscreteStock::get_state_async, stock_mbox.address());
                process.push_downstream.connect(DiscreteStock::add, stock_mbox.address());
                stock.state_emitter.connect(DiscreteParallelProcess::update_state, process_mbox.address());
                Ok(())
            },
            (ComponentModel::ProtoCarStock(stock, stock_mbox), ComponentModel::ProtoCarParallelProcess(process, process_mbox)) => {
                process.req_upstream.connect(DiscreteStock::get_state_async, stock_mbox.address());
                process.withdraw_upstream.connect(DiscreteStock::remove, stock_mbox.address());
                stock.state_emitter.connect(DiscreteParallelProcess::update_state, process_mbox.address());
                Ok(())
            },
            (ComponentModel::ProtoCarStock(stock, stock_mbox), ComponentModel::ProtoCarSink(sink, sink_mbox)) => {
                sink.req_upstream.connect(DiscreteStock::get_state_async, stock_mbox.address());
                sink.withdraw_upstream.connect(DiscreteStock::remove, stock_mbox.address());
                stock.state_emitter.connect(DiscreteSink::update_state, sink_mbox.address());
                Ok(())
            },
            (ComponentModel::BasicEnvironment(controller, controller_mbox), ComponentModel::ProtoCarProcess(process, process_mbox)) => {
                controller.emit_change.connect(DiscreteProcess::update_state, process_mbox.address());
                process.req_environment.connect(BasicEnvironment::get_state_async, controller_mbox.address());
                Ok(())
            },
            (ComponentModel::BasicEnvironment(controller, controller_mbox), ComponentModel::ProtoCarParallelProcess(process, process_mbox)) => {
                controller.emit_change.connect(DiscreteParallelProcess::update_state, process_mbox.address());
                process.req_environment.connect(BasicEnvironment::get_state_async, controller_mbox.address());
                Ok(())
            },
            (ComponentModel::BasicEnvironment(controller, controller_mbox), ComponentModel::ProtoCarSource(source, source_mbox)) => {
                controller.emit_change.connect(DiscreteSource::update_state, source_mbox.address());
                source.req_environment.connect(BasicEnvironment::get_state_async, controller_mbox.address());
                Ok(())
            },
            (ComponentModel::BasicEnvironment(controller, controller_mbox), ComponentModel::ProtoCarSink(sink, sink_mbox)) => {
                controller.emit_change.connect(DiscreteSink::update_state, sink_mbox.address());
                sink.req_environment.connect(BasicEnvironment::get_state_async, controller_mbox.address());
                Ok(())
            },
            (a, b) => Err(format!("No component connection defined from {} to {} (n={:?})", a, b, n).into()),
        }
    }
}

impl CustomLoggerConnection for ComponentLogger { 
    type ComponentType = ComponentModel;
    fn connect_logger(a: &mut Self, b: &mut Self::ComponentType, n: Option<usize>) -> Result<(), Box<dyn Error>> {
        match (a, b, n) {
            (ComponentLogger::ProtoCarProcessLogger(logger), ComponentModel::ProtoCarProcess(process, _), _) => {
                process.log_emitter.connect_sink(&logger.buffer);
                Ok(())
            },
            (ComponentLogger::ProtoCarProcessLogger(logger), ComponentModel::ProtoCarParallelProcess(process, _), _) => {
                process.log_emitter.connect_sink(&logger.buffer);
                Ok(())
            },
            (ComponentLogger::ProtoCarProcessLogger(logger), ComponentModel::ProtoCarSource(process, _), _) => {
                process.log_emitter.connect_sink(&logger.buffer);
                Ok(())
            },
            (ComponentLogger::ProtoCarProcessLogger(logger), ComponentModel::ProtoCarSink(process, _), _) => {
                process.log_emitter.connect_sink(&logger.buffer);
                Ok(())
            },
            (ComponentLogger::ProtoCarStockLogger(logger), ComponentModel::ProtoCarStock(stock, _), _) => {
                stock.log_emitter.connect_sink(&logger.buffer);
                Ok(())
            },
            (a, b, _) => Err(format!("No logger connection defined from {} to {} (n={:?})", a, b, n).into()),
        }
    }
}

fn main() {

    let mut df = DistributionFactory {
        base_seed: 12345,
        next_seed: 0,
    };

    let mut source = ComponentModel::ProtoCarSource(DiscreteSource::new()
        .with_name("Source")
        .with_code("SRC") 
        .with_process_time_distr(Distribution::Constant(15. * 60.)),
        Mailbox::new()
    );

    let mut queue_1 = ComponentModel::ProtoCarStock(DiscreteStock::new()
        .with_name("Queue1")
        .with_code("Q1") 
        .with_low_capacity(0)
        .with_max_capacity(10),
        Mailbox::new()
    );

    let mut process_1 = ComponentModel::ProtoCarProcess(DiscreteProcess::new()
        .with_name("Process1")
        .with_code("P1") 
        .with_process_time_distr(df.create(DistributionConfig::Triangular { min: 10. * 60., max: 25. * 60., mode: 13. * 60. }).unwrap()),
        Mailbox::new()
    );

    let mut queue_2 = ComponentModel::ProtoCarStock(DiscreteStock::new()
        .with_name("Queue2")
        .with_code("Q2") 
        .with_low_capacity(0)
        .with_max_capacity(10),
        Mailbox::new()
    );

    let mut process_par = ComponentModel::ProtoCarParallelProcess(DiscreteParallelProcess::new()
        .with_name("Process2")
        .with_code("P2") 
        .with_process_time_distr(df.create(DistributionConfig::Triangular { min: 10. * 60., max: 25. * 60., mode: 13. * 60. }).unwrap()),
        Mailbox::new()
    );

    let mut queue_3 = ComponentModel::ProtoCarStock(DiscreteStock::new()
        .with_name("Queue3")
        .with_code("Q3") 
        .with_low_capacity(0)
        .with_max_capacity(10),
        Mailbox::new()
    );

    let mut sink = ComponentModel::ProtoCarSink(DiscreteSink::new()
        .with_name("Sink")
        .with_code("SNK") 
        .with_process_time_distr(Distribution::Constant(15. * 60.)),
        Mailbox::new()
    );

    let mut env_controller = ComponentModel::BasicEnvironment(BasicEnvironment::new()
        .with_name("EnvironmentController")
        .with_code("ENV") ,
        Mailbox::new()
    );
    let env_addr = env_controller.get_address();

    connect_components!(&mut source, &mut queue_1).unwrap();
    connect_components!(&mut queue_1, &mut process_1).unwrap();
    connect_components!(&mut process_1, &mut queue_2).unwrap();
    connect_components!(&mut queue_2, &mut process_par).unwrap();
    connect_components!(&mut process_par, &mut queue_3).unwrap();
    connect_components!(&mut queue_3, &mut sink).unwrap();

    connect_components!(&mut env_controller, &mut source).unwrap();
    connect_components!(&mut env_controller, &mut process_1).unwrap();
    connect_components!(&mut env_controller, &mut process_par).unwrap();
    connect_components!(&mut env_controller, &mut sink).unwrap();

    let mut queue_logger = ComponentLogger::ProtoCarStockLogger(DiscreteStockLogger::new("StockLogger".into()));
    let mut process_logger = ComponentLogger::ProtoCarProcessLogger(DiscreteProcessLogger::new("ProcessLogger".into()));
    let mut env_logger = ComponentLogger::BasicEnvironmentLogger(BasicEnvironmentLogger::new("EnvControllerLogger".into()));

    connect_logger!(&mut queue_logger, &mut queue_1).unwrap();
    connect_logger!(&mut queue_logger, &mut queue_2).unwrap();
    connect_logger!(&mut queue_logger, &mut queue_3).unwrap();
    connect_logger!(&mut process_logger, &mut source).unwrap();
    connect_logger!(&mut process_logger, &mut process_1).unwrap();
    connect_logger!(&mut process_logger, &mut process_par).unwrap();
    connect_logger!(&mut process_logger, &mut sink).unwrap();
    connect_logger!(&mut env_logger, &mut env_controller).unwrap();

    let mut sim_builder = SimInit::new();
    sim_builder = register_component!(sim_builder, source);
    sim_builder = register_component!(sim_builder, queue_1);
    sim_builder = register_component!(sim_builder, process_1);
    sim_builder = register_component!(sim_builder, queue_2);
    sim_builder = register_component!(sim_builder, process_par);
    sim_builder = register_component!(sim_builder, queue_3);
    sim_builder = register_component!(sim_builder, sink);
    sim_builder = register_component!(sim_builder, env_controller);

    let start_time = MonotonicTime::try_from_date_time(2025, 7, 1, 0, 0, 0, 0).unwrap();

    let (mut simu, mut sched) = sim_builder.init(start_time).unwrap();

    let event_time = start_time + Duration::from_secs(3600);
    let sched_event = ScheduledEvent::SetEnvironmentState(BasicEnvironmentState::Stopped);
    create_scheduled_event!(&mut sched, &event_time, &sched_event, &env_addr, &mut df);

    let event_time = start_time + Duration::from_secs(7200);
    let sched_event = ScheduledEvent::SetEnvironmentState(BasicEnvironmentState::Normal);
    create_scheduled_event!(&mut sched, &event_time, &sched_event, &env_addr, &mut df);

    simu.step_until(start_time + Duration::from_secs(3 * 24 * 3600)).unwrap();

    let output_dir = "outputs/assembly_line";
    create_dir_all(output_dir).unwrap();
    queue_logger.write_csv(output_dir).unwrap();
    process_logger.write_csv(output_dir).unwrap();
    env_logger.write_csv(output_dir).unwrap();
}