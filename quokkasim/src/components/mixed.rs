use std::time::Duration;
use std::fmt::Debug;

use nexosim::ports::Output;
use serde::Serialize;
use crate::prelude::*;

#[derive(WithMethods)]
pub struct DefaultLoadingProcess<
    ContainerType: Clone + Send + 'static,
    ContainerProcessLogType: Clone + Send + 'static,
    ResourceType: ContArithmetic + Clone + Serialize + Send + 'static,
> {
    // Identification
    pub element_name: String,
    pub element_code: String,
    pub element_type: String,

    // Ports
    pub req_upstream_vehicles: Requestor<(), DiscStockState>,
    pub withdraw_upstream_vehicles: Requestor<(usize, EventId), ContainerType>,

    pub req_upstream_resources: Requestor<(), ResourceType>,
    pub withdraw_upstream_resources: Requestor<(usize, EventId), ResourceType>,

    pub req_downstream: Requestor<(), ContainerType>,
    pub push_downstream: Output<(ContainerType, EventId)>,

    pub log_emitter: Output<ContainerProcessLogType>,

    // Configuration
    pub process_quantity_distr: Distribution,
    pub process_time_distr: Distribution,

    // Runtime state
    pub process_state: Option<(Duration, Vec<ContainerType>)>,

    // Internals
    pub time_to_next_process_event: Option<Duration>,
    pub scheduled_event: Option<(MonotonicTime, ActionKey)>,
    pub next_event_index: u64,
    pub previous_check_time: MonotonicTime,
}

impl<
    ContainerType: Send + 'static,
    ContainerStockState: StockState + Send + 'static,
    ContainerProcessLogType: Clone + Send + 'static,
    // ResourceType: ContArithmetic + Clone + Serialize + Send + 'static,
> Model for DefaultLoadingProcess<
    ContainerType,
    ContainerStockState,
    ContainerProcessLogType,
    // ResourceType,
> {
    fn init(
        mut self,
        ctx: &mut Context<Self>,
    ) -> impl Future<Output = InitializedModel<Self>> + Send {
        async move {
            let source_event_id = EventId(format!(
                "{}_{:06}",
                self.element_code, self.next_event_index
            ));
            self.update_state(source_event_id, ctx).await;
            self.into()
        }
    }
}

// Core
impl<
    ContainerType: Send + 'static,
    ContainerStockState: StockState + Send + 'static,
    ContainerProcessLogType: Clone + Send + 'static,
> DefaultLoadingProcess<
    ContainerType,
    ContainerStockState,
    ContainerProcessLogType,
> {
    fn update_state(
            &mut self,
            source_event_id: EventId,
            cx: &mut Context<Self>,
        ) -> impl Future<Output = ()> {
        async move {
            self.update_state_since_last_update(&mut source_event_id.clone(), cx)
                .await;
            self.update_state_decision_logic(&mut source_event_id.clone(), cx)
                .await;
            self.update_state_for_next_event(&mut source_event_id.clone(), cx)
                .await;
        }
    }

    fn element_name(&self) -> &str {
        &self.element_name
    }

    fn element_code(&self) -> &str {
        &self.element_code
    }

    fn element_type(&self) -> &str {
        &self.element_type
    }

    fn get_next_event_id(&mut self) -> EventId {
        let id = EventId(format!(
            "{}_{:06}",
            self.element_code, self.next_event_index
        ));
        self.next_event_index += 1;
        id
    }

    fn scheduled_event(
        &mut self,
    ) -> &mut Option<(MonotonicTime, ActionKey)> {
        &mut self.scheduled_event
    }

    fn previous_check_time(&mut self) -> &mut MonotonicTime {
        &mut self.previous_check_time
    }

    fn time_to_next_process_event(
        &mut self,
    ) -> &mut Option<Duration> {
        &mut self.time_to_next_process_event
    }
}