use serde::{ser::SerializeStruct, Serialize};

// use quokkasim::prelude::*;
use crate::iron_ore::*;

#[derive(Clone, Debug)]
pub struct Truck { 
    pub ore: Option<IronOre>, 
    pub truck_id: String,
}

impl Default for Truck {
    fn default() -> Self {
        Truck {
            ore: Default::default(),
            truck_id: "1".into()
        }
    }
}

impl Serialize for Truck {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("Truck", 2)?;
        state.serialize_field("ore", &self.ore)?;
        state.serialize_field("truck_id", &self.truck_id)?;
        state.end()
    }
}