use nexosim::ports::Output;
use crate::prelude::*;

enum DiscreteStockLogType<T> {
    Add { balance: u32, added: Vec<T> },
    Remove { balance: u32, removed: Vec<T> },
    StateChange { new_state: DiscreteStockState },
}

enum DiscreteStockState {
    Full { occupied: u32, empty: u32 },
    Normal { occupied: u32, empty: u32 },
    Empty { occupied: u32, empty: u32 },
}

impl StockState for DiscreteStockState {
    fn is_same_state(&self, other: &Self) -> bool {
        match (self, other) {
            (DiscreteStockState::Full { .. }, DiscreteStockState::Full { .. }) => true,
            (DiscreteStockState::Normal { .. }, DiscreteStockState::Normal { .. }) => true,
            (DiscreteStockState::Empty { .. }, DiscreteStockState::Empty { .. }) => true,
            _ => false,
        }
    }
}

pub struct BasicDiscreteStock<T, S: StockState, RecordLogType: Clone + Send + 'static> {

    // Identification
    pub element_name: String,
    pub element_code: String,
    pub element_type: String,

    // Ports
    pub log_emitter: Output<RecordLogType>,
    pub state_emitter: Output<EventId>,

    // Configuration
    pub low_capacity: u32,
    pub max_capacity: u32,

    // Runtime State
    pub resources: VecDequeStock<T>,

    // Internals
    prev_state: Option<S>,
    next_event_index: u64,
}

impl<T, RecordLogType: Clone + Send + 'static> BasicDiscreteStock<T, DiscreteStockState, RecordLogType> {
    fn get_state(&mut self) -> DiscreteStockState {
        let occupied = self.resources.total();
        let empty = self.max_capacity.saturating_sub(occupied);
        if occupied == 0 {
            DiscreteStockState::Empty { occupied, empty }
        } else if empty == 0 {
            DiscreteStockState::Full { occupied, empty }
        } else {
            DiscreteStockState::Normal { occupied, empty }
        }
    }

    fn log_emitter(&mut self) -> &mut Output<RecordLogType> {
        &mut self.log_emitter
    }

    fn get_next_event_id(&mut self) -> EventId {
        let event_id = EventId(format!(
            "{}_{:06}",
            self.element_code, self.next_event_index
        ));
        self.next_event_index += 1;
        event_id
    }

    fn previous_state(&mut self) -> &mut Option<DiscreteStockState> {
        &mut self.prev_state
    }
    fn state_emitter(&mut self) -> &mut Output<EventId> {
        &mut self.state_emitter
    }
    fn log_type_add(&self, balance: u32, resource: Vec<T>) -> DiscreteStockLogType<T> {
        DiscreteStockLogType::Add { balance, added: resource }
    }
    fn log_type_remove(&self, balance: u32, resource: Vec<T>) -> DiscreteStockLogType<T> {
        DiscreteStockLogType::Remove { balance, removed: resource }
    }
    fn log_type_state_change(&self, new_state: DiscreteStockState) -> DiscreteStockLogType<T> {
        DiscreteStockLogType::StateChange { new_state }
    }
}

