use std::{fmt::Debug, time::Duration};
use quokkasim_derive_macros::WithMethods;
use serde::{ser::SerializeStruct, Deserialize, Serialize};

use crate::{delays::{DelayModeChange, DelayModes}, nexosim::{ActionKey, Address, Context, InitializedModel, Model, MonotonicTime, Output, Requestor}, prelude::{VectorStockState, EventId}};


#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum BasicEnvironmentState {
    Normal,
    Stopped,
}

#[derive(Clone)] 
pub struct BasicEnvironmentLog {
    pub time: String,
    pub event_id: EventId,
    pub source_event_id: EventId,
    pub element_name: String,
    pub element_type: String,
    pub event: BasicEnvironmentState,
}

#[derive(WithMethods)]
pub struct BasicEnvironment {
    pub element_name: String,
    pub element_code: String,
    pub state: BasicEnvironmentState,
    pub next_event_index: u64,
    pub log_emitter: Output<BasicEnvironmentLog>,
    pub emit_change: Output<EventId>,
}

impl Default for BasicEnvironment {
    fn default() -> Self {
        BasicEnvironment {
            element_name: "BasicEnvironment".to_string(),
            element_code: "BE_000000".to_string(),
            state: BasicEnvironmentState::Normal,
            next_event_index: 0,
            log_emitter: Output::new(),
            emit_change: Output::new(),
        }
    }
}

impl Model for BasicEnvironment {
    fn init(mut self, cx: &mut Context<Self>) -> impl Future<Output = InitializedModel<Self>> + Send {
        async move {
            self.log(cx.time(), EventId::from_init(), self.state.clone()).await;
            self.into()
        }
    }
}

impl BasicEnvironment {
    pub fn with_state(mut self, state: BasicEnvironmentState) -> Self {
        self.state = state;
        self
    }

    pub fn set_state(&mut self, payload: (BasicEnvironmentState, EventId), cx: &mut Context<Self>) -> impl Future<Output = ()> {
        async move {
            let (state, mut event_id) = payload;
            if self.state != state {
                self.state = state;
                event_id = self.log(cx.time(), event_id, self.state.clone()).await;
                self.emit_change.send(event_id).await;
            }
        }
    }

    pub fn get_state_async(&mut self) -> impl Future<Output = BasicEnvironmentState> + {
        async move {
            self.state.clone()
        }
    }

    fn log(&mut self, now: MonotonicTime, source_event_id: EventId, event: BasicEnvironmentState) -> impl Future<Output = EventId> + Send {
        async move {
            let new_event_id = EventId(format!("{}_{:06}", self.element_code, self.next_event_index));
            let log = BasicEnvironmentLog {
                time: now.to_string(),
                event_id: new_event_id.clone(),
                source_event_id,
                element_name: self.element_name.clone(),
                element_type: "BasicEnvironment".to_string(),
                event,
            };
            self.log_emitter.send(log).await;
            self.next_event_index += 1;

            new_event_id
        }
    }
}