use quokkasim::prelude::*;

#[derive(Debug, Clone)]
pub struct IronOre {
    pub fe: f64,
    pub other_elements: f64,
    pub magnetite: f64,
    pub hematite: f64,
    pub limonite: f64,
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

impl VectorArithmetic<IronOre, f64, f64> for IronOre {
    fn add(&mut self, other: Self) {
        self.fe += other.fe;
        self.other_elements += other.other_elements;
        self.magnetite += other.magnetite;
        self.hematite += other.hematite;
        self.limonite += other.limonite;
    }

    fn subtract(&mut self, quantity: f64) -> IronOre {
        let proportion_removed = quantity / self.total();
        let proportion_remaining = 1.0 - proportion_removed;
        
        self.fe *= proportion_remaining;
        self.other_elements *= proportion_remaining;
        self.magnetite *= proportion_remaining;
        self.hematite *= proportion_remaining;
        self.limonite *= proportion_remaining;

        IronOre {
            fe: self.fe * proportion_remaining,
            other_elements: self.other_elements * proportion_remaining,
            magnetite: self.magnetite * proportion_remaining,
            hematite: self.hematite * proportion_remaining,
            limonite: self.limonite * proportion_remaining,
        }
    }

    // We use the Fe + Other Elements as the 'source of truth' for the total mass
    fn total(&self) -> f64 {
        self.fe + self.other_elements
    }
}