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

