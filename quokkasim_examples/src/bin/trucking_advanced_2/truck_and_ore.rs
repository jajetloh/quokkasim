use std::time::Duration;

use quokkasim::prelude::*;
use crate::ore::*;

#[derive(Debug, Clone)]
pub struct TruckAndOre {
    pub truck_id: u32,
    pub payload: Distribution,
    pub ore_t: f64,
}

pub struct LoadingProcess {
    pub element_name: String,
    pub element_type: String,
    pub req_upstream_trucks: Requestor<(), DiscreteStockState>,
    pub req_upstream_ore: Requestor<(), VectorStockState>,
    pub withdraw_upstream_trucks: Requestor<(TruckAndOre, NotificationMetadata), ()>,
    pub withdraw_upstream_ore: Requestor<(IronOre, NotificationMetadata), f64>,
    pub req_downstream: Requestor<(), DiscreteStockState>,
    pub push_downstream: Output<(TruckAndOre, NotificationMetadata)>,
    
    pub process_state: Option<(Duration, TruckAndOre)>,
    
    pub process_time_distr: Distribution,
    pub time_to_next_event: Option<Duration>,
    next_event_id: u64,
    pub log_emitter: Output<DiscreteStockLog<TruckAndOre>>,
    pub previous_check_time: MonotonicTime,
}

struct TruckAndOreProcessLog {
    
}

impl LoadingProcess {
    
    fn set_previous_check_time(&mut self, time: MonotonicTime) {
        self.previous_check_time = time;
    }

    fn get_time_to_next_event(&mut self) -> &Option<Duration> {
        &self.time_to_next_event
    }

    fn set_time_to_next_event(&mut self, time: Option<Duration>) {
        self.time_to_next_event = time;
    }
    
    fn update_state<'a>(&'a mut self, notif_meta: NotificationMetadata, cx: &'a mut Context<Self>) -> impl Future<Output = ()> + 'a where Self: Model {
        async move {
            self.pre_update_state(&notif_meta, cx).await;
            self.update_state_impl(&notif_meta, cx).await;
            self.post_update_state(&notif_meta, cx).await;
        }
    }

    fn pre_update_state<'a>(&'a mut self, notif_meta: &'a NotificationMetadata, cx: &'a mut Context<Self>) -> impl Future<Output = ()> + 'a where Self: Model {
        async move {}
    }

    fn update_state_impl<'a>(&'a mut self, notif_meta: &'a NotificationMetadata, cx: &'a mut Context<Self>) -> impl Future<Output = ()> + 'a where Self: Model {
        async move {}
    }

    fn post_update_state<'a> (&'a mut self, notif_meta: &'a NotificationMetadata, cx: &'a mut Context<Self>) -> impl Future<Output = ()> + Send + 'a where Self: Model {
        async move {}
    }

    fn log<'a>(&'a mut self, time: MonotonicTime, details: TruckAndOreProcessLog) -> impl Future<Output = ()> + Send {
        async move {

        }
    }
}
