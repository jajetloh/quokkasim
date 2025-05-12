use quokkasim::prelude::*;
use quokkasim::define_model_enums;
use serde_yaml::Sequence;
use std::collections::VecDeque;
use std::{error::Error, time::Duration};
use std::fmt::Debug;

define_model_enums! {
    pub enum ComponentModel<'a> {}
    pub enum ComponentLogger<'a> {}
}

#[derive(Debug, Clone)]
pub enum SequenceStockState {
    Empty { occupied: u32, empty: u32 },
    Normal { occupied: u32, empty: u32 },
    Full { occupied: u32, empty: u32 },
}

impl SequenceStockState {
    pub fn get_name(&self) -> String {
        match self {
            SequenceStockState::Empty { .. } => "Empty".to_string(),
            SequenceStockState::Normal { .. } => "Normal".to_string(),
            SequenceStockState::Full { .. } => "Full".to_string(),
        }
    }
}

impl StateEq for SequenceStockState {
    fn is_same_state(&self, other: &Self) -> bool {
        match (self, other) {
            (SequenceStockState::Empty { .. }, SequenceStockState::Empty { ..  }) => true,
            (SequenceStockState::Normal { .. }, SequenceStockState::Normal { .. }) => true,
            (SequenceStockState::Full { .. }, SequenceStockState::Full { .. }) => true,
            _ => false,
        }
    }
}

pub struct SequenceStock<T> {
    pub element_name: String,
    pub element_type: String,
    pub sequence: SeqDeque<T>,
    pub log_emitter: Output<SequenceStockLog<T>>,
    pub state_emitter: Output<NotificationMetadata>,
    pub low_capacity: f64,
    pub max_capacity: f64,
    pub prev_state: Option<SequenceStockState>,
    next_event_id: u64,
}
impl<T: Clone + Default + Send> Default for SequenceStock<T> {
    fn default() -> Self {
        SequenceStock {
            element_name: "SequenceStock".to_string(),
            element_type: "SequenceStock".to_string(),
            sequence: SeqDeque::default(),
            log_emitter: Output::new(),
            state_emitter: Output::new(),
            low_capacity: 0.0,
            max_capacity: 100.0,
            prev_state: None,
            next_event_id: 0,
        }
    }
}

#[derive(Debug, Clone)]
struct SeqDeque<T> {
    deque: VecDeque<T>,
}

impl<T> VectorArithmetic for SeqDeque<T> {
    fn add(&self, other: &Self) -> Self {
        todo!()
    }
    fn subtract_parts(&self, other: f64) -> SubtractParts<Self> {
        todo!()
    }
    fn total(&self) -> f64 {
        todo!()
    }
}

impl<T: Default> Default for SeqDeque<T> {
    fn default() -> Self {
        SeqDeque {
            deque: VecDeque::new(),
        }
    }
}

impl<T: Clone + Debug + Default + Send> Stock<SeqDeque<T>, T, ()> for SequenceStock<T> where Self: Model {
    type StockState = SequenceStockState;
    fn get_state(&mut self) -> Self::StockState {
        let occupied = self.sequence.total() as u32;
        let empty = self.max_capacity as u32;
        if self.sequence.total() == 0.0 { // TODO: fix
            SequenceStockState::Empty { occupied, empty }
        } else if self.sequence.total() >= self.max_capacity {
            SequenceStockState::Full { occupied, empty }
        } else {
            SequenceStockState::Normal { occupied, empty }
        }
    }
    fn get_previous_state(&mut self) -> &Option<Self::StockState> {
        &self.prev_state
    }
    fn set_previous_state(&mut self) {
        self.prev_state = Some(self.get_state());
    }
    fn get_resource(&self) -> &SeqDeque<T> {
        &self.sequence
    }
    fn add_impl<'a>(&'a mut self, payload: &'a (T, NotificationMetadata), cx: &'a mut Context<Self>) -> impl Future<Output = ()> + 'a {
        async move {
            self.prev_state = Some(self.get_state());
            self.sequence.deque.push_back(payload.0.clone());
        }
    }
    fn remove_impl<'a>(&'a mut self, payload: &'a ((), NotificationMetadata), cx: &'a mut Context<Self>) -> impl Future<Output = SeqDeque<T>> + 'a {
        async move {
            self.prev_state = Some(self.get_state());
            let mut deque = VecDeque::<T>::new();
            deque.push_front(self.sequence.deque.pop_front());
            SeqDeque {
                deque: VecDeque::<T>::from(&[self.sequence.deque.pop_front()])
            }
        }
    }


}

impl<'a> CustomComponentConnection for ComponentModel<'a> {
    fn connect_components(a: Self, b: Self) -> Result<(), Box<dyn Error>> {
        Err(format!("connect_components not implemented from {} to {}", a, b).into())
    }
}

impl<'a> CustomLoggerConnection<'a> for ComponentLogger<'a> {
    type ComponentType = ComponentModel<'a>;
    fn connect_logger(a: Self, b: Self::ComponentType) -> Result<(), Box<dyn Error>> {
        Err(format!("connect_logger not implemented from {} to {}", a, b).into())
    }
}

fn main() {
    
}