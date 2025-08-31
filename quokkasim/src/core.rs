use std::{fmt::Debug, time::Duration};
use quokkasim_derive_macros::WithMethods;
use serde::{ser::SerializeStruct, Deserialize, Serialize};

use crate::{common::{EventId}, components::{continuous_traits::ContinuousResource, environment::{BasicEnvironment, BasicEnvironmentState}}, delays::{DelayModeChange, DelayModes}, nexosim::{ActionKey, Address, Context, InitializedModel, Model, MonotonicTime, Output, Requestor}};


pub trait ToLogRecord<DetailsType, LogType> {
    fn to_record(&mut self, source_event_id: EventId, event_id: EventId, details: DetailsType) -> LogType;
}

pub trait StockState {
    fn is_same_state(&self, other: &Self) -> bool;
}