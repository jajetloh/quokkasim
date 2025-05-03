use std::{error::Error, fs::File};

use csv::WriterBuilder;
use nexosim::ports::EventBuffer;
use quokkasim::prelude::{VectorStockLog, QueueStockLog};
use serde::{Deserialize, Serialize};

use crate::components::process::TruckingProcessLog;

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
    buffer: EventBuffer<VectorStockLog>,
}

impl Logger for ArrayStockLogger {
    type RecordType = VectorStockLog;
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