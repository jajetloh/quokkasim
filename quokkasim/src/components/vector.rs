use futures::{future::join_all};
use nexosim::{model::Model, ports::{EventQueue, Output, Requestor}};
use serde::{ser::SerializeStruct, Serialize};
use tai_time::MonotonicTime;
use std::{fmt::Debug, time::Duration};

use crate::prelude::*;

/**
 * Stock
 */

#[derive(Debug, Clone)]
pub enum VectorStockState {
    Empty { occupied: f64, empty: f64 },
    Normal { occupied: f64, empty: f64 },
    Full { occupied: f64, empty: f64 },
}

