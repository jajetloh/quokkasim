use std::{error::Error, fs::File};

use clap::Parser;
use csv::WriterBuilder;
use indexmap::IndexMap;
use log::warn;
use nexosim::{ports::EventBuffer, simulation::Address};
use quokkasim::{core::{DistributionConfig, DistributionFactory, Mailbox, Process, ResourceAdd, Stock}, prelude::{ArrayResource, ArrayStock, ArrayStockLog, QueueStockLog}};
use serde::{Deserialize, Serialize};

use crate::components::{process::{DumpingProcess, LoadingProcess, TruckMovementProcess, TruckingProcessLog}, stock::TruckStock, ComponentModel};


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
        let mut stock = ArrayStock::new()
            .with_name(self.name.clone());
        stock.resource.add(ArrayResource { vec: self.vec });
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
        Ok(ComponentModel::ArrayStock(stock, mbox, addr))
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
        (ComponentModel::ArrayStock(mut stock_model, stock_mbox, stock_addr), ComponentModel::LoadingProcess(mut loading, loading_mbox, loading_addr)) => {
            loading.req_upstreams.0.connect(ArrayStock::get_state, &stock_addr);
            loading.withdraw_upstreams.0.connect(ArrayStock::remove, &stock_addr);
            stock_model.state_emitter.connect(LoadingProcess::check_update_state, &loading_addr);
            Ok((ComponentModel::ArrayStock(stock_model, stock_mbox, stock_addr),
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
        (ComponentModel::DumpingProcess(mut dumping, dumping_mbox, dumping_addr), ComponentModel::ArrayStock(mut stock_model, stock_mbox, stock_addr)) => {
            dumping.req_downstreams.0.connect(ArrayStock::get_state, &stock_addr);
            dumping.push_downstreams.0.connect(ArrayStock::add, &stock_addr);
            stock_model.state_emitter.connect(DumpingProcess::check_update_state, &dumping_addr);
            Ok((ComponentModel::DumpingProcess(dumping, dumping_mbox, dumping_addr),
            ComponentModel::ArrayStock(stock_model, stock_mbox, stock_addr)))
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

/// Trucking simulation command line options.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct CLIArgs {
    /// The base seed used for random distributions.
    #[arg(long, default_value = "1")]
    pub seed: String,

    /// The number of trucks to simulate.
    #[arg(long, default_value = "2")]
    pub num_trucks: usize,

    /// The simulation duration in seconds.
    #[arg(long, default_value = "21600")]
    pub sim_duration_secs: f64,
}

pub struct ParsedArgs {
    pub seed: u64,
    pub num_trucks: usize,
    pub sim_duration_secs: f64,
}

/// Parse a seed string such as "0..6" or "0..=7" into a vector of u64 values.
pub fn parse_seed_range(seed_str: &str) -> Result<Vec<u64>, String> {
    if let Some(idx) = seed_str.find("..=") {
        let (start, end) = seed_str.split_at(idx);
        let end = &end[3..]; // skip "..="
        let start: u64 = start
            .trim()
            .parse()
            .map_err(|e| format!("Invalid start seed '{}': {}", start.trim(), e))?;
        let end: u64 = end
            .trim()
            .parse()
            .map_err(|e| format!("Invalid end seed '{}': {}", end.trim(), e))?;
        Ok((start..=end).collect())
    } else if let Some(idx) = seed_str.find("..") {
        let (start, end) = seed_str.split_at(idx);
        let end = &end[2..]; // skip ".."
        let start: u64 = start
            .trim()
            .parse()
            .map_err(|e| format!("Invalid start seed '{}': {}", start.trim(), e))?;
        let end: u64 = end
            .trim()
            .parse()
            .map_err(|e| format!("Invalid end seed '{}': {}", end.trim(), e))?;
        Ok((start..end).collect())
    } else {
        seed_str
            .trim()
            .parse::<u64>()
            .map(|v| vec![v])
            .map_err(|e| format!("Invalid seed value '{}': {}", seed_str, e))
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
    ArrayStock(Address<ArrayStock>),
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

pub enum EventLogger {
    TruckingProcessLogger(TruckingProcessLogger),
    QueueStockLogger(QueueStockLogger),
    ArrayStockLogger(ArrayStockLogger),
}

impl EventLogger {
    pub fn get_name(&self) -> &String {
        match self {
            EventLogger::TruckingProcessLogger(x) => x.get_name(),
            EventLogger::QueueStockLogger(x) => x.get_name(),
            EventLogger::ArrayStockLogger(x) => x.get_name(),
        }
    }
}

pub struct TruckingProcessLogger {
    pub name: String,
    pub buffer: EventBuffer<<Self as Logger>::RecordType>,
}

impl Logger for TruckingProcessLogger {

    type RecordType = TruckingProcessLog;
    fn get_name(&self) -> &String {
        &self.name
    }
    fn get_buffer(&self) -> &EventBuffer<Self::RecordType> {
        &self.buffer
    }
    fn write_csv(self, dir: String) -> Result<(), Box<dyn Error>> {
        // let file = File::create(path)?;
        let file = File::create(format!("{}/{}.csv", dir, self.name))?;
        let mut writer = WriterBuilder::new()
            .has_headers(true)
            .from_writer(file);
        self.buffer.for_each(|log| {
            writer.serialize(log).expect("Failed to write log record to CSV file");
        });
        writer.flush()?;
        Ok(())
    }
}

impl TruckingProcessLogger {
    fn new(name: String, buffer_size: usize) -> Self {
        TruckingProcessLogger {
            name,
            buffer: EventBuffer::with_capacity(buffer_size),
        }
    }
}

pub struct QueueStockLogger {
    name: String,
    buffer: EventBuffer<QueueStockLog>,
}

impl Logger for QueueStockLogger {
    type RecordType = QueueStockLog;
    fn get_name(&self) -> &String {
        &self.name
    }
    fn get_buffer(&self) -> &EventBuffer<Self::RecordType> {
        &self.buffer
    }
    fn write_csv(self, dir: String) -> Result<(), Box<dyn Error>> {
        let file = File::create(format!("{}/{}.csv", dir, self.name))?;
        let mut writer = WriterBuilder::new()
            .has_headers(true)
            .from_writer(file);
        self.buffer.for_each(|log| {
            writer.serialize(log).expect("Failed to write log record to CSV file");
        });
        writer.flush()?;
        Ok(())
    }
}

impl QueueStockLogger {
    fn new(name: String, buffer_size: usize) -> Self {
        QueueStockLogger {
            name,
            buffer: EventBuffer::with_capacity(buffer_size),
        }
    }
}

pub struct ArrayStockLogger {
    name: String,
    buffer: EventBuffer<ArrayStockLog>,
}

impl Logger for ArrayStockLogger {
    type RecordType = ArrayStockLog;
    fn get_name(&self) -> &String {
        &self.name
    }
    fn get_buffer(&self) -> &EventBuffer<Self::RecordType> {
        &self.buffer
    }
    fn write_csv(self, dir: String) -> Result<(), Box<dyn Error>> {
        // TODO: turn this into a derive macro
        let file = File::create(format!("{}/{}.csv", dir, self.name))?;
        let mut writer = WriterBuilder::new()
            .has_headers(true)
            .from_writer(file);
        self.buffer.for_each(|log| {
            writer.serialize(log).expect("Failed to write log record to CSV file");
        });
        writer.flush()?;
        Ok(())
    }
}

impl ArrayStockLogger {
    fn new(name: String, buffer_size: usize) -> Self {
        ArrayStockLogger {
            name,
            buffer: EventBuffer::with_capacity(buffer_size),
        }
    }
}

pub trait Logger {
    type RecordType: Serialize;
    fn get_name(&self) -> &String;
    fn get_buffer(&self) -> &EventBuffer<Self::RecordType>;
    fn write_csv(self, dir: String) -> Result<(), Box<dyn Error>>;
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggerConfig {
    name: String,
    record_type: String,
    max_length: usize,
    log_path: String,
}

pub fn create_logger(config: LoggerConfig) -> Result<EventLogger, Box<dyn Error>> {
    let (name, log_type, max_length) = (config.name, config.record_type, config.max_length);
    match log_type.as_str() {
        "TruckingProcessLog" | "TruckAndOreStockLog" => {
            let buffer = TruckingProcessLogger::new(name, max_length);
            Ok(EventLogger::TruckingProcessLogger(buffer))
        },
        "QueueStockLog" => {
            let buffer = QueueStockLogger::new(name, max_length);
            Ok(EventLogger::QueueStockLogger(buffer))
        },
        "ArrayStockLog" => {
            let buffer = ArrayStockLogger::new(name, max_length);
            Ok(EventLogger::ArrayStockLogger(buffer))
        },
        _ => Err(format!("Unknown log type: {}", log_type).into()),
    }
}