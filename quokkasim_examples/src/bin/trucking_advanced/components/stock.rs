use std::time::Duration;

use indexmap::{IndexMap, IndexSet};
use nexosim::{model::Context, time::MonotonicTime};
use quokkasim::{core::{ResourceAdd, ResourceRemove, StateEq}, define_stock, prelude::QueueStockLog};
use serde::{ser::SerializeStruct, Serialize};

use super::TruckAndOre;


#[derive(Debug, Clone)]
pub enum TruckStockState {
    Empty,
    Normal(IndexSet<i32>),
}

impl StateEq for TruckStockState {
    fn is_same_state(&self, other: &Self) -> bool {
        match (self, other) {
            (TruckStockState::Empty, TruckStockState::Empty) => true,
            (TruckStockState::Normal(x), TruckStockState::Normal(y)) => x == y,
            _ => false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TruckAndOreStockLog {
    pub time: String,
    pub element_name: String,
    pub element_type: String,
    pub details: TruckAndOreStockLogDetails,
}

impl Serialize for TruckAndOreStockLog {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("TruckAndOreStockLog", 10)?;
        state.serialize_field("time", &self.time)?;
        state.serialize_field("element_name", &self.element_name)?;
        state.serialize_field("element_type", &self.element_type)?;
        let (log_type, truck_id, occupied, empty, x0, x1, x2, x3, x4): (
            &str, i32, f64, f64, f64, f64, f64, f64, f64,
        ) = match self.details {
            TruckAndOreStockLogDetails::StockAdded { truck_id, total, empty, contents,} => (
                "StockAdded".into(), truck_id, total, empty, contents[0], contents[1], contents[2], contents[3], contents[4],
            ),
            TruckAndOreStockLogDetails::StockRemoved { truck_id, total, empty, contents,} => (
                "StockRemoved".into(), truck_id, total, empty, contents[0], contents[1], contents[2], contents[3], contents[4],
            ),
        };
        state.serialize_field("event_type", &log_type)?;
        state.serialize_field("truck_id", &truck_id)?;
        state.serialize_field("occupied", &occupied)?;
        state.serialize_field("empty", &empty)?;
        state.serialize_field("x0", &x0)?;
        state.serialize_field("x1", &x1)?;
        state.serialize_field("x2", &x2)?;
        state.serialize_field("x3", &x3)?;
        state.serialize_field("x4", &x4)?;
        state.end()
    }
}

#[derive(Debug, Clone)]
pub enum TruckAndOreStockLogDetails {
    StockAdded {
        truck_id: i32,
        total: f64,
        empty: f64,
        contents: [f64; 5],
    },
    StockRemoved {
        truck_id: i32,
        total: f64,
        empty: f64,
        contents: [f64; 5],
    },
}

define_stock!(
    /// TruckStock
    name = TruckStock,
    resource_type = TruckAndOreMap,
    initial_resource = Default::default(),
    add_type = Option<TruckAndOre>,
    remove_type = Option<TruckAndOre>,
    remove_parameter_type = i32,
    state_type = TruckStockState,
    fields = {
        low_capacity: f64,
        max_capacity: f64,
        remaining_durations: IndexMap<i32, Duration>
    },
    get_state_method = |x: &Self| -> TruckStockState {
        if x.resource.trucks.is_empty() {
            TruckStockState::Empty
        } else {
            TruckStockState::Normal(IndexSet::from_iter(x.resource.trucks.clone().into_keys()))
        }
    },
    check_update_method = |x: &mut Self, cx: &mut Context<Self>| {
    },
    log_record_type = QueueStockLog,
    log_method = |x: &'a mut Self, time: MonotonicTime, log_type: String| {
        async move {
            let log = QueueStockLog {
                time: time.to_chrono_date_time(0).unwrap().to_string(),
                element_name: x.element_name.clone(),
                element_type: x.element_type.clone(),
                log_type,
                occupied: x.resource.trucks.len() as i32,
                empty: 999,
                state: "".into(),
                contents: "".into(),
            };
            x.log_emitter.send(log).await;
        }
    }
);

impl TruckStock {
    pub fn remove_any(
        &mut self,
        data: ((), NotificationMetadata),
        cx: &mut Context<Self>,
    ) -> impl Future<Output = Option<TruckAndOre>> {
        async move {
            self.prev_state = Some(self.get_state().await);
            let truck = match self.resource.trucks.pop() {
                Some((_, truck)) => Some(truck),
                None => {
                    None
                }
            };
            self.log(data.1.time, "RemoveAny".into()).await;
            self.check_update_state(data.1, cx).await;
            truck
        }
    }
}


pub struct TruckAndOreMap {
    pub trucks: IndexMap<i32, TruckAndOre>,
}

impl ResourceAdd<Option<TruckAndOre>> for TruckAndOreMap {
    fn add(&mut self, truck_and_ore: Option<TruckAndOre>) {
        match truck_and_ore {
            Some(item) => {
                self.trucks.insert(item.truck, item);
            }
            None => {}
        }
    }
}

impl ResourceRemove<i32, Option<TruckAndOre>> for TruckAndOreMap {
    fn sub(&mut self, id: i32) -> Option<TruckAndOre> {
        self.trucks.swap_remove(&id)
    }
}

impl Default for TruckAndOreMap {
    fn default() -> Self {
        TruckAndOreMap {
            trucks: IndexMap::new(),
        }
    }
}