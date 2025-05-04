use std::time::Duration;

use indexmap::IndexSet;

use crate::components::vector::{VectorResource, VectorStock};
use crate::core::MonotonicTime;

enum VectorPacketStockState {
    Empty,
    Normal(IndexSet<String>)
}

#[derive(Debug, Clone)]
pub struct VectorPacketResource {
    pub id: String,
    pub vector: VectorResource,
}

pub struct VectorPacketProcessLog {
    pub time: String,
    pub element_name: String,
    pub element_type: String,
    pub event_id: String,
    pub process_data: VectorPacketProcessLogType,
}

pub enum VectorPacketProcessLogType {
    CombineStart { id: String, quantity: f64, vector: [f64; 5] },
    CombineSuccess { id: String, quantity: f64, vector: [f64; 5] },
}

pub enum VectorPacketCombinerProcessState {
    Idle,
    Processing { id: String, previous_check_time: MonotonicTime, time_until_done: Duration },
}

// define_combiner_process!(
//     /// Process which adds vector resource to an existing vector packet
//     name = VectorPacketCombinerProcess,
//     inflow_stock_state_types = (VectorStockState, VectorPacketStockState),
//     resource_in_types = (VectorResource, Option<VectorPacketStockState>),
//     resource_in_parameter_types = (f64, ()),
//     outflow_stock_state_type = VectorPacketStockState,
//     resource_out_type = Option<VectorPacketStockState>,
//     resource_out_parameter_type = Option<VectorPacketStockState>,
//     check_update_method = |mut x: Self, time: MonotonicTime| {
//         async move {
//             // First resolve Loading state, if applicable
//             match x.state.clone() {
//                 LoadingProcessState::Loading { truck, previous_check_time, time_until_done } => {
//                     let elapsed_time = time.duration_since(previous_check_time);
//                     let new_time_until_done = time_until_done.saturating_sub(elapsed_time);
//                     let new_previous_check_time = time;

//                     if new_time_until_done.is_zero() {
//                         x.log(time, TruckingProcessLogType::LoadSuccess { truck_id: truck.truck,  tonnes: truck.ore.total(), components: truck.ore.vec } ).await;
//                         x.log_truck_stock(time, TruckAndOreStockLogDetails::StockAdded { truck_id: truck.truck, total: truck.ore.total(), empty: 999., contents: truck.ore.vec }).await;
//                         x.push_downstream.send((Some(truck.clone()), NotificationMetadata {
//                             time,
//                             element_from: x.element_name.clone(),
//                             message: "Truck and ore".into(),
//                         })).await;
//                         x.state = LoadingProcessState::Idle;
//                     } else {
//                         x.state = LoadingProcessState::Loading { truck, previous_check_time: new_previous_check_time, time_until_done: new_time_until_done };
//                         x.time_to_next_event_counter = Some(time_until_done);
//                         return x;
//                     }
//                 },
//                 LoadingProcessState::Idle => {}
//             }

//             // Then execute new load
//             let us_material_state: VectorStockState = x.req_upstreams.0.send(()).await.next().unwrap();
//             let us_truck_state: TruckStockState = x.req_upstreams.1.send(()).await.next().unwrap();

//             match (&us_material_state, &us_truck_state) {
//                 (VectorStockState::Normal { .. } | VectorStockState::Full { .. }, TruckStockState::Normal { .. }) => {
//                     let mut truck = x.withdraw_upstreams.1.send(((), NotificationMetadata {
//                         time,
//                         element_from: x.element_name.clone(),
//                         message: "Truck request".into(),
//                     })).await.next().unwrap();
//                     let material = x.withdraw_upstreams.0.send((x.load_quantity_dist.as_mut().unwrap().sample(), NotificationMetadata {
//                         time,
//                         element_from: x.element_name.clone(),
//                         message: "Material request".into(),
//                     })).await.next().unwrap();

//                     match truck.take() {
//                         Some(mut truck) => {
//                             let truck_id = truck.truck;
//                             truck.ore = material.clone();
//                             let time_until_done = Duration::from_secs_f64(x.load_time_dist_secs.as_mut().unwrap().sample());
//                             x.state = LoadingProcessState::Loading { truck, previous_check_time: time.clone(), time_until_done };
//                             x.log(time, TruckingProcessLogType::LoadStart { truck_id,  tonnes: material.total(), components: material.vec.clone() } ).await;
//                             x.time_to_next_event_counter = Some(time_until_done);
//                         },
//                         None => {
//                             x.state = LoadingProcessState::Idle;
//                             x.log(time, TruckingProcessLogType::LoadStartFailed { reason: "No trucks available" }).await;
//                             x.time_to_next_event_counter = None;
//                         }
//                     }
//                 },
//                 (VectorStockState::Empty { .. }, _) => {
//                     x.log(time, TruckingProcessLogType::LoadStartFailed { reason: "No material available" }).await;
//                     x.time_to_next_event_counter = None;
//                 },
//                 (_, TruckStockState::Empty) => {
//                     x.log(time, TruckingProcessLogType::LoadStartFailed { reason: "No trucks available" }).await;
//                     x.time_to_next_event_counter = None;
//                 }
//             }
//             x
//         }
//     },
//     fields = {
//         state: VectorPacketCombinerProcessState,
//         stock_emitter: Output<VectorPacketStockLog>,
//         process_time_dist: Option<Distribution>,
//         process_quantity_dist: Option<Distribution>
//     },
//     log_record_type = VectorPacketProcessLog,
//     log_method = |x: &'a mut Self, time: MonotonicTime, log_type: String| {
//         async move {
//             let log = VectorPacketProcessLog {
//                 time: time.to_chrono_date_time(0).unwrap().to_string(),
//                 element_name: x.element_name.clone(),
//                 element_type: x.element_type.clone(),
//                 log_type,
//                 truck_id: 0,
//                 tonnes: 0.,
//                 components: vec![],
//             };
//             x.log_emitter.send(log).await;
//         }
//     }
// )