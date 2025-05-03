use std::error::Error;
use indexmap::IndexMap;
use log::warn;
use nexosim::simulation::Address;
use quokkasim::{core::{DistributionConfig, DistributionFactory, Mailbox, Process, ResourceAdd, Stock}, prelude::{VectorResource, VectorStock}};
use serde::Deserialize;

use crate::{components::{process::{DumpingProcess, LoadingProcess, TruckMovementProcess}, stock::TruckStock, ComponentModel}, loggers::{EventLogger, Logger, LoggerConfig}};


#[derive(Debug, Clone, Deserialize)]
pub struct ArrayStockConfig {
    name: String,
    vec: [f64; 5],
    low_capacity: f64,
    max_capacity: f64,
    loggers: Vec<String>,
}

impl ArrayStockConfig {
    fn create_component(&self, df: &mut DistributionFactory, loggers: &mut IndexMap<String, EventLogger>) -> Result<ComponentModel, Box<dyn Error>> {
        let mut stock = VectorStock::new()
            .with_name(self.name.clone());
        stock.resource.add(VectorResource { vec: self.vec });
        stock.low_capacity = self.low_capacity;
        stock.max_capacity = self.max_capacity;
        self.loggers.iter().for_each(|logger_name| {
            match loggers.get(logger_name) {
                Some(EventLogger::ArrayStockLogger(logger)) => stock.log_emitter.connect_sink(logger.get_buffer()),
                _ => {
                    // TODO: Better error handling here
                    warn!("No logger called {} found for ArrayStock {}", logger_name, self.name);
                }
            }
        });
        let mbox = Mailbox::new();
        let addr = mbox.address();
        Ok(ComponentModel::VectorStock(stock, mbox, addr))
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct TruckStockConfig {
    name: String,
    loggers: Vec<String>,
}

impl TruckStockConfig {
    fn create_component(&self, df: &mut DistributionFactory, loggers: &mut IndexMap<String, EventLogger>) -> Result<ComponentModel, Box<dyn Error>> {
        let mut stock = TruckStock::new()
            .with_name(self.name.clone());
        self.loggers.iter().for_each(|logger_name| {
            match loggers.get(logger_name) {
                Some(EventLogger::QueueStockLogger(logger)) => stock.log_emitter.connect_sink(logger.get_buffer()),
                _ => {
                    warn!("No logger called {} found for TruckStock {}", logger_name, self.name);
                }
            }
        });
        let mbox = Mailbox::new();
        let addr = mbox.address();
        Ok(ComponentModel::TruckStock(stock, mbox, addr))
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoadingProcessConfig {
    name: String,
    load_time_dist_secs: DistributionConfig,
    load_quantity_dist: DistributionConfig,
    loggers: Vec<String>,
}

impl LoadingProcessConfig {
    fn create_component(&self, df: &mut DistributionFactory, loggers: &mut IndexMap<String, EventLogger>) -> Result<ComponentModel, Box<dyn Error>> {
        let mut loading = LoadingProcess::new()
            .with_name(self.name.clone())
            .with_load_time_dist_secs(Some(df.create(self.load_time_dist_secs.clone())?))
            .with_load_quantity_dist(Some(df.create(self.load_quantity_dist.clone())?));

        self.loggers.iter().for_each(|logger_name| {
            match loggers.get(logger_name) {
                Some(EventLogger::TruckingProcessLogger(logger)) => loading.log_emitter.connect_sink(logger.get_buffer()),
                _ => {
                    warn!("No logger called {} found for LoadingProcess {}", logger_name, self.name);
                }
            }
        });

        let mbox = Mailbox::new();
        let addr = mbox.address();
        Ok(ComponentModel::LoadingProcess(loading, mbox, addr))
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct DumpingProcessConfig {
    name: String,
    dump_time_dist_secs: DistributionConfig,
    loggers: Vec<String>,
}

impl DumpingProcessConfig {
    fn create_component(&self, df: &mut DistributionFactory, loggers: &mut IndexMap<String, EventLogger>) -> Result<ComponentModel, Box<dyn Error>> {
        let mut dumping = DumpingProcess::new()
            .with_name(self.name.clone())
            .with_dump_time_dist_secs(Some(df.create(self.dump_time_dist_secs.clone())?));

        self.loggers.iter().for_each(|logger_name| {
            match loggers.get(logger_name) {
                Some(EventLogger::TruckingProcessLogger(logger)) => dumping.log_emitter.connect_sink(logger.get_buffer()),
                _ => {
                    warn!("No logger called {} found for DumpingProcess {}", logger_name, self.name);
                }
            }
        });

        let mbox = Mailbox::new();
        let addr = mbox.address();
        Ok(ComponentModel::DumpingProcess(dumping, mbox, addr))
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct TruckMovementProcessConfig {
    name: String,
    travel_time_dist_secs: DistributionConfig,
}

impl TruckMovementProcessConfig {
    fn create_component(&self, df: &mut DistributionFactory, loggers: &mut IndexMap<String, EventLogger>) -> Result<ComponentModel, Box<dyn Error>> {
        let movement = TruckMovementProcess::new()
            .with_name(self.name.clone())
            .with_travel_time_dist_secs(Some(df.create(self.travel_time_dist_secs.clone())?));
        let mbox = Mailbox::new();
        let addr = mbox.address();
        Ok(ComponentModel::TruckMovementProcess(movement, mbox, addr))
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ConnectionConfig {
    pub upstream: String,
    pub downstream: String,
}

pub fn connect_components(
    comp1: ComponentModel,
    comp2: ComponentModel,
) -> Result<(ComponentModel, ComponentModel), Box<dyn Error>> {
    let comp1_name = comp1.get_name().clone();
    let comp2_name = comp2.get_name().clone();
    match (comp1, comp2) {
        (ComponentModel::TruckStock(mut stock_model, stock_mbox, stock_addr), ComponentModel::LoadingProcess(mut loading, loading_mbox, loading_addr)) => {
            loading.req_upstreams.1.connect(TruckStock::get_state, &stock_addr);
            loading.withdraw_upstreams.1.connect(TruckStock::remove_any, &stock_addr);
            stock_model.state_emitter.connect(LoadingProcess::check_update_state, &loading_addr);
            Ok((ComponentModel::TruckStock(stock_model, stock_mbox, stock_addr),
            ComponentModel::LoadingProcess(loading, loading_mbox, loading_addr)))
        },
        (ComponentModel::VectorStock(mut stock_model, stock_mbox, stock_addr), ComponentModel::LoadingProcess(mut loading, loading_mbox, loading_addr)) => {
            loading.req_upstreams.0.connect(VectorStock::get_state, &stock_addr);
            loading.withdraw_upstreams.0.connect(VectorStock::remove, &stock_addr);
            stock_model.state_emitter.connect(LoadingProcess::check_update_state, &loading_addr);
            Ok((ComponentModel::VectorStock(stock_model, stock_mbox, stock_addr),
            ComponentModel::LoadingProcess(loading, loading_mbox, loading_addr)))
        },
        (ComponentModel::LoadingProcess(mut loading, loading_mbox, loading_addr), ComponentModel::TruckStock(mut stock_model, stock_mbox, stock_addr)) => {
            loading.req_downstream.connect(TruckStock::get_state, &stock_addr);
            loading.push_downstream.connect(TruckStock::add, &stock_addr);
            stock_model.state_emitter.connect(LoadingProcess::check_update_state, &loading_addr);
            Ok((ComponentModel::LoadingProcess(loading, loading_mbox, loading_addr),
            ComponentModel::TruckStock(stock_model, stock_mbox, stock_addr)))
        },
        (ComponentModel::TruckStock(mut stock_model, stock_mbox, stock_addr), ComponentModel::TruckMovementProcess(mut movement, movement_mbox, movement_addr)) => {
            movement.req_upstream.connect(TruckStock::get_state, &stock_addr);
            movement.withdraw_upstream.connect(TruckStock::remove, &stock_addr); 
            stock_model.state_emitter.connect(TruckMovementProcess::check_update_state, &movement_addr);
            Ok((ComponentModel::TruckStock(stock_model, stock_mbox, stock_addr),
            ComponentModel::TruckMovementProcess(movement, movement_mbox, movement_addr)))
        },
        (ComponentModel::TruckMovementProcess(mut movement, movement_mbox, movement_addr), ComponentModel::TruckStock(mut stock_model, stock_mbox, stock_addr)) => {
            movement.req_downstream.connect(TruckStock::get_state, &stock_addr);
            movement.push_downstream.connect(TruckStock::add, &stock_addr);
            stock_model.state_emitter.connect(TruckMovementProcess::check_update_state, &movement_addr);
            Ok((ComponentModel::TruckMovementProcess(movement, movement_mbox, movement_addr),
            ComponentModel::TruckStock(stock_model, stock_mbox, stock_addr)))
        },
        (ComponentModel::TruckStock(mut stock_model, stock_mbox, stock_addr), ComponentModel::DumpingProcess(mut dumping, dumping_mbox, dumping_addr)) => {
            dumping.req_upstream.connect(TruckStock::get_state, &stock_addr);
            dumping.withdraw_upstream.connect(TruckStock::remove_any, &stock_addr);
            stock_model.state_emitter.connect(DumpingProcess::check_update_state, &dumping_addr);
            Ok((ComponentModel::TruckStock(stock_model, stock_mbox, stock_addr),
            ComponentModel::DumpingProcess(dumping, dumping_mbox, dumping_addr)))
        },
        (ComponentModel::DumpingProcess(mut dumping, dumping_mbox, dumping_addr), ComponentModel::VectorStock(mut stock_model, stock_mbox, stock_addr)) => {
            dumping.req_downstreams.0.connect(VectorStock::get_state, &stock_addr);
            dumping.push_downstreams.0.connect(VectorStock::add, &stock_addr);
            stock_model.state_emitter.connect(DumpingProcess::check_update_state, &dumping_addr);
            Ok((ComponentModel::DumpingProcess(dumping, dumping_mbox, dumping_addr),
            ComponentModel::VectorStock(stock_model, stock_mbox, stock_addr)))
        },
        (ComponentModel::DumpingProcess(mut dumping, dumping_mbox, dumping_addr), ComponentModel::TruckStock(mut stock_model, stock_mbox, stock_addr)) => {
            dumping.req_downstreams.1.connect(TruckStock::get_state, &stock_addr);
            dumping.push_downstreams.1.connect(TruckStock::add, &stock_addr);
            stock_model.state_emitter.connect(DumpingProcess::check_update_state, &dumping_addr);
            Ok((ComponentModel::DumpingProcess(dumping, dumping_mbox, dumping_addr),
            ComponentModel::TruckStock(stock_model, stock_mbox, stock_addr)))
        },
        _ => Err(format!("Connection error: Implementation does not exist for instances {} to {}", comp1_name, comp2_name).into()),
    }
}

impl ComponentConfig {
    pub fn create_component(&self, df: &mut DistributionFactory, loggers: &mut IndexMap<String, EventLogger>) -> Result<ComponentModel, Box<dyn Error>> {
        match self {
            ComponentConfig::ArrayStock(config) => config.create_component(df, loggers),
            ComponentConfig::TruckStock(config) => config.create_component(df, loggers),
            ComponentConfig::LoadingProcess(config) => config.create_component(df, loggers),
            ComponentConfig::DumpingProcess(config) => config.create_component(df, loggers),
            ComponentConfig::TruckMovementProcess(config) => config.create_component(df, loggers),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum ComponentConfig {
    ArrayStock(ArrayStockConfig),
    TruckStock(TruckStockConfig),
    LoadingProcess(LoadingProcessConfig),
    DumpingProcess(DumpingProcessConfig),
    TruckMovementProcess(TruckMovementProcessConfig),
}

pub enum ComponentModelAddress {
    ArrayStock(Address<VectorStock>),
    TruckStock(Address<TruckStock>),
    LoadingProcess(Address<LoadingProcess>),
    DumpingProcess(Address<DumpingProcess>),
    TruckMovementProcess(Address<TruckMovementProcess>),
}


#[derive(Clone, Debug, Deserialize)]
pub struct ModelConfig {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub truck_init_location: String,
    pub loggers: Vec<LoggerConfig>,
    pub components: Vec<ComponentConfig>,
    pub connections: Vec<ConnectionConfig>,
}