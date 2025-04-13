use crate::{
    common::{Distribution, EventLog, EventLogger, NotificationMetadata}, core::{ResourceAdd, ResourceMultiply, ResourceRemove, StateEq}, define_combiner_process, define_process, define_sink, define_source, define_splitter_process, define_stock
};
use nexosim::{model::Context, ports::Output, time::MonotonicTime};
use serde::{ser::SerializeStruct, Serialize};
use serde_json::to_string;

/**
 * This module is based around the `ArrayResource` type, which holds an array of 5 f64 values.
 * Processing is performed instantaneously by Process-type components.
 */

#[derive(Debug, Clone)]
pub enum ArrayStockState {
    Empty {
        occupied: f64,
        remaining_capacity: f64,
    },
    Normal {
        occupied: f64,
        remaining_capacity: f64,
    },
    Full {
        occupied: f64,
        remaining_capacity: f64,
    },
}

impl StateEq for ArrayStockState {
    fn is_same_state(&self, other: &Self) -> bool {
        match (self, other) {
            (ArrayStockState::Empty { .. }, ArrayStockState::Empty { .. }) => true,
            (ArrayStockState::Normal { .. }, ArrayStockState::Normal { .. }) => true,
            (ArrayStockState::Full { .. }, ArrayStockState::Full { .. }) => true,
            _ => false,
        }
    }
}

/// A resource type that contains an array of 5 f64 values.
#[derive(Debug, Clone)]
pub struct ArrayResource {
    pub vec: [f64; 5],
}

impl ArrayResource {
    pub fn total(&self) -> f64 {
        self.vec.iter().sum()
    }
}

impl Default for ArrayResource {
    fn default() -> Self {
        ArrayResource { vec: [0_f64; 5] }
    }
}

impl ResourceAdd<Self> for ArrayResource {
    fn add(&mut self, item: Self) {
        self.vec
            .iter_mut()
            .zip(item.vec.iter())
            .for_each(|(a, b)| *a += b);
    }
}

impl ResourceRemove<f64, ArrayResource> for ArrayResource {
    fn sub(&mut self, qty: f64) -> ArrayResource {
        // Removes proportionally from each element of the array
        let proportion = qty / self.total();
        let removed = self.vec.map(|x| x * proportion);
        self.vec
            .iter_mut()
            .zip(removed.iter())
            .for_each(|(a, b)| *a -= b);
        ArrayResource { vec: removed }
    }
}

impl ResourceRemove<ArrayResource, ArrayResource> for ArrayResource {
    fn sub(&mut self, qty: ArrayResource) -> ArrayResource {
        self.vec
            .iter_mut()
            .zip(qty.vec.iter())
            .for_each(|(a, b)| *a -= b);
        qty
    }
}

impl ResourceMultiply<f64> for ArrayResource {
    fn mul(&mut self, qty: f64) -> ArrayResource {
        let mut new_resource = self.clone();
        new_resource.vec.iter_mut().for_each(|x| *x *= qty);
        new_resource
    }
}

#[derive(Serialize, Clone, Debug)]
pub struct ArrayStockLog {
    pub time: String,
    pub element_name: String,
    pub element_type: String,
    pub log_type: String,
    pub occupied: f64,
    pub remaining_capacity: f64,
    pub state: String,
    pub x0: f64,
    pub x1: f64,
    pub x2: f64,
    pub x3: f64,
    pub x4: f64,
}

define_stock!(
    /// Stock for the `ArrayResource` type.
    name = ArrayStock,
    resource_type = ArrayResource,
    initial_resource = ArrayResource { vec: [0.0; 5] },
    add_type = ArrayResource,
    remove_type = ArrayResource,
    remove_parameter_type = f64,
    state_type = ArrayStockState,
    fields = {
        low_capacity: f64,
        max_capacity: f64
    },
    get_state_method = |x: &Self| -> ArrayStockState {
        let total = x.resource.total();
        if total <= x.low_capacity {
            ArrayStockState::Empty {
                occupied: total,
                remaining_capacity: x.max_capacity - total,
            }
        } else if total < x.max_capacity {
            ArrayStockState::Normal {
                occupied: total,
                remaining_capacity: x.max_capacity - total,
            }
        } else {
            ArrayStockState::Full {
                occupied: total,
                remaining_capacity: 0.0,
            }
        }
    },
    check_update_method = |x: &mut Self, cx: &mut Context<Self>| {
    },
    log_record_type = ArrayStockLog,
    log_method = |x: &'a mut Self, time: MonotonicTime, log_type: String| {
        async move {
            let state = x.get_state().await;
            let log = ArrayStockLog {
                time: time.to_chrono_date_time(0).unwrap().to_string(),
                element_name: x.element_name.clone(),
                element_type: x.element_type.clone(),
                log_type,
                occupied: match state { 
                    ArrayStockState::Empty { occupied, .. } => occupied,
                    ArrayStockState::Normal { occupied, .. } => occupied,
                    ArrayStockState::Full { occupied, .. } => occupied,
                },
                remaining_capacity: match state {
                    ArrayStockState::Empty { remaining_capacity, .. } => remaining_capacity,
                    ArrayStockState::Normal { remaining_capacity, .. } => remaining_capacity,
                    ArrayStockState::Full { remaining_capacity, .. } => remaining_capacity,
                },
                state: match state {
                    ArrayStockState::Empty { .. } => "Empty".to_string(),
                    ArrayStockState::Normal { .. } => "Normal".to_string(),
                    ArrayStockState::Full { .. } => "Full".to_string(),
                },
                x0: x.resource.vec[0],
                x1: x.resource.vec[1],
                x2: x.resource.vec[2],
                x3: x.resource.vec[3],
                x4: x.resource.vec[4],
            };
            x.log_emitter.send(log).await;
        }
    }
);

#[derive(Clone, Debug)]
pub struct ArrayProcessLog {
    pub time: String,
    pub element_name: String,
    pub element_type: String,
    pub process_data: ArrayProcessLogType,
}

impl Serialize for ArrayProcessLog {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("ArrayProcessLog", 6)?;
        state.serialize_field("time", &self.time)?;
        state.serialize_field("element_name", &self.element_name)?;
        state.serialize_field("element_type", &self.element_type)?;
        let mut event_type: Option<&'static str> = None;
        let mut quantity: Option<f64> = None;
        let mut reason: Option<&'static str> = None;
        match &self.process_data {
            ArrayProcessLogType::SourceSuccess { quantity: q } => {
                event_type = Some("SourceSuccess");
                quantity = Some(*q);
                reason = None;
            }
            ArrayProcessLogType::SourceFailure { reason: r } => {
                event_type = Some("SourceFailure");
                quantity = None;
                reason = Some(r);
            }
            ArrayProcessLogType::ProcessSuccess { quantity: q } => {
                event_type = Some("ProcessSuccess");
                quantity = Some(*q);
                reason = None;
            }
            ArrayProcessLogType::ProcessFailure { reason: r } => {
                event_type = Some("ProcessFailure");
                quantity = None;
                reason = Some(r);
            }
            ArrayProcessLogType::SinkSuccess { quantity: q } => {
                event_type = Some("SinkSuccess");
                quantity = Some(*q);
                reason = None;
            }
            ArrayProcessLogType::SinkFailure { reason: r } => {
                event_type = Some("SinkFailure");
                quantity = None;
                reason = Some(r);
            }
        }
        state.serialize_field("event_type", &event_type).unwrap();
        state.serialize_field("quantity", &quantity).unwrap();
        state.serialize_field("reason", &reason).unwrap();
        state.end()
    }
}

#[derive(Clone, Debug)]
pub enum ArrayProcessLogType  {
    SourceSuccess { quantity: f64 },
    SourceFailure { reason: &'static str },
    ProcessSuccess { quantity: f64 },
    ProcessFailure { reason: &'static str },
    SinkSuccess { quantity: f64 },
    SinkFailure { reason: &'static str },
}

define_source!(
    /// Source for the `ArrayResource` type.
    name = ArraySource,
    resource_type = ArrayResource,
    stock_state_type = ArrayStockState,
    add_type = ArrayResource,
    add_parameter_type = f64,
    create_method = |mut source: &mut Self, x: f64| -> ArrayResource {
        let proportion = x / source.component_split.total();
        let mut new_resource = ArrayResource { vec: [0.0; 5] };
        new_resource.vec.iter_mut().zip(source.component_split.vec.iter()).for_each(|(a, b)| *a = b * proportion);
        new_resource
    },
    check_update_method = |mut x: Self, time: MonotonicTime| {
        async move {
            let ds_state = x.req_downstream.send(()).await.next();
            match ds_state {
                Some(ArrayStockState::Empty { .. } | ArrayStockState::Normal { .. }) => {
                    let qty = x.create_quantity_dist.sample();
                    let new_resource = x.create(qty.clone());
                    x.push_downstream.send((new_resource.clone(), NotificationMetadata {
                        time,
                        element_from: x.element_name.clone(),
                        message: "New resource created".to_string(),
                    })).await;
                    x.log(time, ArrayProcessLogType::SourceSuccess { quantity: qty }).await;
                },
                Some(ArrayStockState::Full { .. }) => {
                    x.log(time, ArrayProcessLogType::SourceFailure { reason: "Downstream is full" }).await;
                },
                None => {
                    x.log(time, ArrayProcessLogType::SourceFailure { reason: "No downstream found" }).await;
                }
            };
            x
        }
    },
    fields = {
        component_split: ArrayResource,
        create_quantity_dist: Distribution
    },
    log_record_type = ArrayProcessLog,
    log_method = |x: &'a mut Self, time: MonotonicTime, details: ArrayProcessLogType| {
        async move {
            // let state = x.get_state().await;
            let log = ArrayProcessLog {
                time: time.to_chrono_date_time(0).unwrap().to_string(),
                element_name: x.element_name.clone(),
                element_type: x.element_type.clone(),
                process_data: details,
                
            };
            x.log_emitter.send(log).await;
        }
    },
    log_method_parameter_type = ArrayProcessLogType
);

define_sink!(
    /// Sink for the `ArrayResource` type.
    name = ArraySink,
    resource_type = ArrayResource,
    stock_state_type = ArrayStockState,
    subtract_type = ArrayResource,
    subtract_parameters_type = f64,
    check_update_method = |mut sink: Self, time: MonotonicTime| {
        async move {
            let us_state = sink.req_upstream.send(()).await.next();
            match us_state {
                Some(ArrayStockState::Full { .. } | ArrayStockState::Normal { .. }) => {
                    let sink_qty = sink.destroy_quantity_dist.sample();
                    let removed = sink.withdraw_upstream.send((sink_qty, NotificationMetadata {
                        time,
                        element_from: sink.element_name.clone(),
                        message: "Resource removed".to_string(),
                    })).await.collect::<Vec<_>>();
                    sink.log(time, ArrayProcessLogType::SinkSuccess { quantity: sink_qty }).await;
                },
                Some(ArrayStockState::Empty { .. }) => {
                    sink.log(time, ArrayProcessLogType::SinkFailure { reason: "Upstream is empty" }).await;
                },
                None => {
                    sink.log(time, ArrayProcessLogType::SinkFailure { reason: "Upstream is not connected" }).await;
                }
            };
            sink
        }
    },
    fields = {
        destroy_quantity_dist: Distribution
    },
    log_record_type = ArrayProcessLog,
    log_method = |x: &'a mut Self, time: MonotonicTime, details: ArrayProcessLogType| {
        async move {
            // let state = x.get_state().await;
            let log = ArrayProcessLog {
                time: time.to_chrono_date_time(0).unwrap().to_string(),
                element_name: x.element_name.clone(),
                element_type: x.element_type.clone(),
                process_data: details,
                
            };
            x.log_emitter.send(log).await;
        }
    },
    log_method_parameter_type = ArrayProcessLogType
);

define_process!(
    /// Process for the `ArrayResource` type.
    name = ArrayProcess,
    stock_state_type = ArrayStockState,
    resource_in_type = ArrayResource,
    resource_in_parameter_type = f64,
    resource_out_type = ArrayResource,
    resource_out_parameter_type = ArrayResource,
    check_update_method = |mut x: Self, time: MonotonicTime| {
        async move {

            let us_state = x.req_upstream.send(()).await.next();
            let ds_state = x.req_downstream.send(()).await.next();

            match (&us_state, &ds_state) {
                (
                    Some(ArrayStockState::Normal {..}) | Some(ArrayStockState::Full {..}),
                    Some(ArrayStockState::Empty {..}) | Some(ArrayStockState::Normal {..}),
                ) => {
                    let process_quantity = x.process_quantity_dist.as_mut().unwrap_or_else(
                        || panic!("Process quantity dist not defined!")
                    ).sample();
                    let moved = x.withdraw_upstream.send((process_quantity, NotificationMetadata {
                        time,
                        element_from: x.element_name.clone(),
                        message: format!("Withdrawing quantity {:?}", process_quantity),
                    })).await.next().unwrap();

                    x.push_downstream.send((moved.clone(), NotificationMetadata {
                        time,
                        element_from: x.element_name.clone(),
                        message: format!("Depositing quantity {:?} ({:?})", process_quantity, moved),
                    })).await;

                    x.log(time, ArrayProcessLogType::ProcessSuccess { quantity: process_quantity }).await;
                },
                (Some(ArrayStockState::Empty {..} ), _) => {
                    x.log(time, ArrayProcessLogType::ProcessFailure { reason: "Upstream is empty" }).await;
                },
                (None, _) => {
                    x.log(time, ArrayProcessLogType::ProcessFailure { reason: "Upstream is not connected" }).await;
                },
                (_, None) => {
                    x.log(time, ArrayProcessLogType::ProcessFailure { reason: "Downstream is not connected" }).await;
                },
                (_, Some(ArrayStockState::Full {..} )) => {
                    x.log(time, ArrayProcessLogType::ProcessFailure { reason: "Downstream is full" }).await;
                },
            }
            x.time_to_next_event_counter = Duration::from_secs_f64(x.process_duration_secs_dist.as_mut().unwrap_or_else(
                || panic!("Process duration distribution not set!")
            ).sample());
            x
        }
    },
    fields = {
        process_quantity_dist: Option<Distribution>,
        process_duration_secs_dist: Option<Distribution>
    },
    log_record_type = ArrayProcessLog,
    log_method = |x: &'a mut Self, time: MonotonicTime, details: ArrayProcessLogType| {
        async move {
            // let state = x.get_state().await;
            let log = ArrayProcessLog {
                time: time.to_chrono_date_time(0).unwrap().to_string(),
                element_name: x.element_name.clone(),
                element_type: x.element_type.clone(),
                process_data: details,
            };
            x.log_emitter.send(log).await;
        }
    },
    log_method_parameter_type = ArrayProcessLogType
);

define_combiner_process!(
    /// Combiner process for the `ArrayResource` type.
    name = ArrayCombinerProcess,
    inflow_stock_state_types = (ArrayStockState, ArrayStockState),
    resource_in_types = (ArrayResource, ArrayResource),
    resource_in_parameter_types = (f64, f64),
    outflow_stock_state_type = ArrayStockState,
    resource_out_type = ArrayResource,
    resource_out_parameter_type = (),
    check_update_method = |mut x: Self, time: MonotonicTime| {
        async move {
            let us_states = (x.req_upstreams.0.send(()).await.next(), x.req_upstreams.1.send(()).await.next());
            let ds_state = x.req_downstream.send(()).await.next();

            match (&us_states.0, &us_states.1, &ds_state) {
                (
                    Some(ArrayStockState::Normal { occupied: occupied_1, .. } ) | Some(ArrayStockState::Full { occupied: occupied_1, .. } ),
                    Some(ArrayStockState::Normal { occupied: occupied_2, .. } ) | Some(ArrayStockState::Full { occupied: occupied_2, .. } ),
                    Some(ArrayStockState::Empty { remaining_capacity, .. } ) | Some(ArrayStockState::Normal { remaining_capacity, .. } ),
                ) => {
                    
                    let process_quantity = x.process_quantity_dist.as_mut().unwrap_or_else(
                        || panic!("Process quantity dist not defined!")
                    ).sample().min(*occupied_1 + *occupied_2).min(*remaining_capacity);

                    let qty1: ArrayResource = x.withdraw_upstreams.0.send((process_quantity, NotificationMetadata {
                        time,
                        element_from: x.element_name.clone(),
                        message: "Withdrawing item".into(),
                    })).await.next().unwrap();

                    let qty2: ArrayResource = x.withdraw_upstreams.1.send((process_quantity, NotificationMetadata {
                        time,
                        element_from: x.element_name.clone(),
                        message: "Withdrawing item".into(),
                    })).await.next().unwrap();

                    let mut total = qty1.clone();
                    total.add(qty2);

                    x.push_downstream.send((total.clone(), NotificationMetadata {
                        time,
                        element_from: x.element_name.clone(),
                        message: "Processing complete".into(),
                    })).await;
                    x.log(time, ArrayProcessLogType::ProcessSuccess { quantity: process_quantity }).await;
                },
                (_, _, Some(ArrayStockState::Full {..} )) => {
                    x.log(time, ArrayProcessLogType::ProcessFailure { reason: "Downstream is full" }).await;
                },
                (_, _, None) => {
                    x.log(time, ArrayProcessLogType::ProcessFailure { reason: "Downstream is not connected" }).await;
                },
                (None, _, _) => {
                    x.log(time, ArrayProcessLogType::ProcessFailure { reason: "Upstream 0 is not connected" }).await;
                }
                (_, None, _) => {
                    x.log(time, ArrayProcessLogType::ProcessFailure { reason: "Upstream 1 is not connected" }).await;
                },
                (Some(ArrayStockState::Empty {..} ), _, _) => {
                    x.log(time, ArrayProcessLogType::ProcessFailure { reason: "Upstream 0 is empty" }).await;
                }
                (_, Some(ArrayStockState::Empty {..} ), _) => {
                    x.log(time, ArrayProcessLogType::ProcessFailure { reason: "Upstream 1 is empty" }).await;
                }
            };
            x.time_to_next_event_counter = Duration::from_secs_f64(x.process_duration_secs_dist.as_mut().unwrap_or_else(
                || panic!("Process duration distribution not set!")
            ).sample());
            x
        }
    },
    fields = {
        process_quantity_dist: Option<Distribution>,
        process_duration_secs_dist: Option<Distribution>
    },
    log_record_type = ArrayProcessLog,
    log_method = |x: &'a mut Self, time: MonotonicTime, details: ArrayProcessLogType| {
        async move {
            // let state = x.get_state().await;
            let log = ArrayProcessLog {
                time: time.to_chrono_date_time(0).unwrap().to_string(),
                element_name: x.element_name.clone(),
                element_type: x.element_type.clone(),
                process_data: details,
                
            };
            x.log_emitter.send(log).await;
        }
    },
    log_method_parameter_type = ArrayProcessLogType
);

define_splitter_process!(
    /// Splitter process for the `ArrayResource` type.
    name = ArraySplitterProcess,
    inflow_stock_state_type = ArrayStockState,
    resource_in_type = ArrayResource,
    resource_in_parameter_type = f64,
    outflow_stock_state_types = (ArrayStockState, ArrayStockState),
    resource_out_types = (ArrayResource, ArrayResource),
    resource_out_parameter_types = (ArrayResource, ArrayResource),
    check_update_method = |mut x: Self, time: MonotonicTime| {
        async move {
            let us_state = x.req_upstream.send(()).await.next();
            let ds_states = (x.req_downstreams.0.send(()).await.next(), x.req_downstreams.1.send(()).await.next());

            match (&us_state, &ds_states.0, &ds_states.1) {
                (
                    Some(ArrayStockState::Normal { occupied, .. } ) | Some(ArrayStockState::Full { occupied, .. } ),
                    Some(ArrayStockState::Empty { remaining_capacity: remaining_capacity_1, .. } ) | Some(ArrayStockState::Normal { remaining_capacity: remaining_capacity_1, .. } ),
                    Some(ArrayStockState::Empty { remaining_capacity: remaining_capacity_2, .. } ) | Some(ArrayStockState::Normal { remaining_capacity: remaining_capacity_2, .. } ),
                ) => {

                    let process_quantity = x.process_quantity_dist.as_mut().unwrap_or_else(
                        || panic!("Process quantity dist not defined!")
                    ).sample().min(*occupied).min(*remaining_capacity_1 + *remaining_capacity_2);

                    let processed_resource: ArrayResource = x.withdraw_upstream.send((process_quantity, NotificationMetadata {
                        time,
                        element_from: x.element_name.clone(),
                        message: "Withdrawing item".into(),
                    })).await.next().unwrap();

                    let qty1 = processed_resource.clone().mul(0.5);
                    let qty2 = processed_resource.clone().mul(0.5);

                    x.push_downstreams.0.send((qty1.clone(), NotificationMetadata {
                        time,
                        element_from: x.element_name.clone(),
                        message: "Processing complete".into(),
                    })).await;
                    x.push_downstreams.1.send((qty2.clone(), NotificationMetadata {
                        time,
                        element_from: x.element_name.clone(),
                        message: "Processing complete".into(),
                    })).await;

                    x.log(time, ArrayProcessLogType::ProcessSuccess { quantity: process_quantity }).await;
                },
                (Some(ArrayStockState::Empty {..} ), _, _) => {
                    x.log(time, ArrayProcessLogType::ProcessFailure { reason: "Upstream is empty" }).await;
                },
                (None, _, _) => {
                    x.log(time, ArrayProcessLogType::ProcessFailure { reason: "Upstream is not connected" }).await;
                },
                (_, None, _) => {
                    x.log(time, ArrayProcessLogType::ProcessFailure { reason: "Downstream 0 is not connected" }).await;
                },
                (_, _, None) => {
                    x.log(time, ArrayProcessLogType::ProcessFailure { reason: "Downstream 1 is not connected" }).await;
                },
                (_, Some(ArrayStockState::Full {..} ), _) => {
                    x.log(time, ArrayProcessLogType::ProcessFailure { reason: "Downstream 0 is full" }).await;
                },
                (_, _, Some(ArrayStockState::Full {..} )) => {
                    x.log(time, ArrayProcessLogType::ProcessFailure { reason: "Downstream 1 is full" }).await;
                },
            };
            x.time_to_next_event_counter = Duration::from_secs_f64(x.process_duration_secs_dist.as_mut().unwrap_or_else(
                || panic!("Process duration distribution not set!")
            ).sample());
            x
        }
    },
    fields = {
        process_quantity_dist: Option<Distribution>,
        process_duration_secs_dist: Option<Distribution>
    },
    log_record_type = ArrayProcessLog,
    log_method = |x: &'a mut Self, time: MonotonicTime, details: ArrayProcessLogType| {
        async move {
            // let state = x.get_state().await;
            let log = ArrayProcessLog {
                time: time.to_chrono_date_time(0).unwrap().to_string(),
                element_name: x.element_name.clone(),
                element_type: x.element_type.clone(),
                process_data: details,
                
            };
            x.log_emitter.send(log).await;
        }
    },
    log_method_parameter_type = ArrayProcessLogType
);
