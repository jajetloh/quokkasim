use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Default)]
/// A short, lightweight identifier for an event. Useful for understanding causal flow of events via log files.
pub struct EventMetadata {
    pub source_name: String,
    pub source_code: String,
    pub index: u64,
}

impl Serialize for EventMetadata {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let s = format!("{}_{:06}", self.source_code, self.index);
        serializer.serialize_str(&s)
    }
}

impl EventMetadata {
    pub fn from_init() -> EventMetadata {
        EventMetadata {
            source_name: "INIT".to_string(),
            source_code: "INIT".to_string(),
            index: 0,
        }
    }

    pub fn from_scheduler() -> EventMetadata {
        EventMetadata {
            source_name: "SCHEDULER".to_string(),
            source_code: "SCH".to_string(),
            index: 0,
        }
    }

    pub fn next(&mut self) -> EventMetadata {
        self.index += 1;
        self.clone()
    }
}

pub trait StockState {
    fn is_same_state(&self, other: &Self) -> bool;
}

#[derive(Debug, Clone)]
pub struct UndefinedConnectionError {
    pub from: String,
    pub from_n: Option<usize>,
    pub to: String,
    pub to_n: Option<usize>,
    pub message: String,
}

#[derive(Debug, Clone)]
pub enum Error {
    UndefinedConnection(UndefinedConnectionError),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::UndefinedConnection(err) => write!(f, "Undefined connection: {}", err.message),
        }
    }
}

// this is the missing bit
impl std::error::Error for Error {}