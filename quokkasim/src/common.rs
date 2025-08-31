use std::{error::Error, fmt::{Display, Formatter, Result as FmtResult}, time::Duration};
use indexmap::IndexMap;
use rand::{rngs::SmallRng, SeedableRng};
use rand_distr::{Distribution as _, Exp, Normal, Triangular, Uniform};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
/// A short, lightweight identifier for an event. Very useful for understanding causal flow of events via log files.
/// Conventionally of the form `PROC_123456`, with a prefix uniquely identifying the process, and suffix an auto-incrementing number.
pub struct EventId(pub String);

impl EventId {
    pub fn from_init() -> EventId {
        EventId("INIT_000000".to_string())
    }

    pub fn from_scheduler() -> EventId {
        EventId("SCH_000000".to_string())
    }
}
