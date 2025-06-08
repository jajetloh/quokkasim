#[allow(clippy::manual_async_fn)]

use quokkasim::prelude::*;
use serde::{ser::SerializeStruct, Serialize};

#[derive(Debug, Clone)]
pub struct IronOre {
    pub fe: f64,
    pub other_elements: f64,
    pub magnetite: f64,
    pub hematite: f64,
    pub limonite: f64,
}

impl Serialize for IronOre {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("IronOre", 5)?;
        state.serialize_field("fe", &self.fe)?;
        state.serialize_field("other_elements", &self.other_elements)?;
        state.serialize_field("magnetite", &self.magnetite)?;
        state.serialize_field("hematite", &self.hematite)?;
        state.serialize_field("limonite", &self.limonite)?;
        state.end()
    }
}

impl Default for IronOre {
    fn default() -> Self {
        IronOre {
            fe: 0.0,
            other_elements: 0.0,
            magnetite: 0.0,
            hematite: 0.0,
            limonite: 0.0,
        }
    }
}


impl ResourceAdd<IronOre> for IronOre {
    fn add(&mut self, other: Self) {
        self.fe += other.fe;
        self.other_elements += other.other_elements;
        self.magnetite += other.magnetite;
        self.hematite += other.hematite;
        self.limonite += other.limonite;
    }
}

impl ResourceRemove<f64, IronOre> for IronOre {
    fn remove(&mut self, quantity: f64) -> Self {
        let proportion_removed = quantity / self.total();
        let proportion_remaining = 1.0 - proportion_removed;
        
        let removed = IronOre {
            fe: self.fe * proportion_removed,
            other_elements: self.other_elements * proportion_removed,
            magnetite: self.magnetite * proportion_removed,
            hematite: self.hematite * proportion_removed,
            limonite: self.limonite * proportion_removed,
        };

        self.fe *= proportion_remaining;
        self.other_elements *= proportion_remaining;
        self.magnetite *= proportion_remaining;
        self.hematite *= proportion_remaining;
        self.limonite *= proportion_remaining;

        removed
    }
}

impl ResourceTotal<f64> for IronOre {
    fn total(&self) -> f64 {
        self.fe + self.other_elements
    }
}

impl ResourceMultiply<f64> for IronOre {
    fn multiply(&mut self, factor: f64) {
        self.fe *= factor;
        self.other_elements *= factor;
        self.magnetite *= factor;
        self.hematite *= factor;
        self.limonite *= factor;
    }
}