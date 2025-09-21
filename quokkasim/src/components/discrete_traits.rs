use serde::Serialize;
use std::{fmt::Debug, time::Duration};

use crate::{
    common::{EventId, StockState, ToLogRecord},
    components::environment::BasicEnvironmentState,
    delays::DelayModes,
    distributions::Distribution,
    nexosim::{ActionKey, Context, Model, MonotonicTime, Output, Requestor},
    prelude::ContinuousStockState,
};

pub trait DiscreteArithmetic {
    fn add
}