use crate::{
    common::{Distribution, Logger}, core::{ResourceAdd, ResourceMultiply, ResourceRemove, StateEq}, define_combiner_process, define_process, define_sink, define_source, define_splitter_process, define_stock
};
use nexosim::{model::Context, ports::EventBuffer, time::MonotonicTime};
use serde::{ser::SerializeStruct, Serialize};

/**
 * This module is based around the `VectorResource` type, which holds an array of 5 f64 values.
 * Processing is performed instantaneously by Process-type components.
 */

#[derive(Debug, Clone)]
pub enum VectorStockState {
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

impl StateEq for VectorStockState {
    fn is_same_state(&self, other: &Self) -> bool {
        match (self, other) {
            (VectorStockState::Empty { .. }, VectorStockState::Empty { .. }) => true,
            (VectorStockState::Normal { .. }, VectorStockState::Normal { .. }) => true,
            (VectorStockState::Full { .. }, VectorStockState::Full { .. }) => true,
            _ => false,
        }
    }
}

/// A resource type that contains an array of 5 f64 values.
#[derive(Debug, Clone)]
pub struct VectorResource {
    pub vec: [f64; 5],
}

impl VectorResource {
    pub fn total(&self) -> f64 {
        self.vec.iter().sum()
    }
}

impl Default for VectorResource {
    fn default() -> Self {
        VectorResource { vec: [0_f64; 5] }
    }
}

impl ResourceAdd<Self> for VectorResource {
    fn add(&mut self, item: Self) {
        self.vec
            .iter_mut()
            .zip(item.vec.iter())
            .for_each(|(a, b)| *a += b);
    }
}

impl ResourceRemove<f64, VectorResource> for VectorResource {
    fn sub(&mut self, qty: f64) -> VectorResource {
        // Removes proportionally from each element of the vector
        let proportion = qty / self.total();
        let removed = self.vec.map(|x| x * proportion);
        self.vec
            .iter_mut()
            .zip(removed.iter())
            .for_each(|(a, b)| *a -= b);
        VectorResource { vec: removed }
    }
}

impl ResourceRemove<VectorResource, VectorResource> for VectorResource {
    fn sub(&mut self, qty: VectorResource) -> VectorResource {
        self.vec
            .iter_mut()
            .zip(qty.vec.iter())
            .for_each(|(a, b)| *a -= b);
        qty
    }
}

impl ResourceMultiply<f64> for VectorResource {
    fn mul(&mut self, qty: f64) -> VectorResource {
        let mut new_resource = self.clone();
        new_resource.vec.iter_mut().for_each(|x| *x *= qty);
        new_resource
    }
}

pub struct VectorStockLogger {
    pub buffer: EventBuffer<VectorStockLog>,
    pub name: String,
}

impl Logger for VectorStockLogger {
    type RecordType = VectorStockLog;
    fn get_name(&self) -> &String {
        &self.name
    }
    fn get_buffer(&self) -> &EventBuffer<Self::RecordType> {
        &self.buffer
    }
    fn write_csv(self, dir: String) -> Result<(), Box<dyn std::error::Error>> {
        let file = std::fs::File::create(format!("{}/{}.csv", dir, self.name))?;
        let mut writer = csv::WriterBuilder::new()
            .has_headers(true)
            .from_writer(file);
        self.buffer.for_each(|log| {
            writer.serialize(log).expect("Failed to write log record to CSV file");
        });
        writer.flush()?;
        Ok(())
    }
}

impl VectorStockLogger {
    pub fn new(name: String, capacity: usize) -> Self {
        VectorStockLogger {
            buffer: EventBuffer::with_capacity(capacity),
            name,
        }
    }
}

#[derive(Serialize, Clone, Debug)]
pub struct VectorStockLog {
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
    /// Stock for the `VectorResource` type.
    name = VectorStock,
    resource_type = VectorResource,
    initial_resource = VectorResource { vec: [0.0; 5] },
    add_type = VectorResource,
    remove_type = VectorResource,
    remove_parameter_type = f64,
    state_type = VectorStockState,
    fields = {
        low_capacity: f64,
        max_capacity: f64
    },
    get_state_method = |x: &Self| -> VectorStockState {
        let total = x.resource.total();
        if total <= x.low_capacity {
            VectorStockState::Empty {
                occupied: total,
                remaining_capacity: x.max_capacity - total,
            }
        } else if total < x.max_capacity {
            VectorStockState::Normal {
                occupied: total,
                remaining_capacity: x.max_capacity - total,
            }
        } else {
            VectorStockState::Full {
                occupied: total,
                remaining_capacity: 0.0,
            }
        }
    },
    check_update_method = |x: &mut Self, cx: &mut Context<Self>| {
    },
    log_record_type = VectorStockLog,
    log_method = |x: &'a mut Self, time: MonotonicTime, log_type: String| {
        async move {
            let state = x.get_state().await;
            let log = VectorStockLog {
                time: time.to_chrono_date_time(0).unwrap().to_string(),
                element_name: x.element_name.clone(),
                element_type: x.element_type.clone(),
                log_type,
                occupied: match state { 
                    VectorStockState::Empty { occupied, .. } => occupied,
                    VectorStockState::Normal { occupied, .. } => occupied,
                    VectorStockState::Full { occupied, .. } => occupied,
                },
                remaining_capacity: match state {
                    VectorStockState::Empty { remaining_capacity, .. } => remaining_capacity,
                    VectorStockState::Normal { remaining_capacity, .. } => remaining_capacity,
                    VectorStockState::Full { remaining_capacity, .. } => remaining_capacity,
                },
                state: match state {
                    VectorStockState::Empty { .. } => "Empty".to_string(),
                    VectorStockState::Normal { .. } => "Normal".to_string(),
                    VectorStockState::Full { .. } => "Full".to_string(),
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

pub struct VectorProcessLogger {
    pub buffer: EventBuffer<VectorProcessLog>,
    pub name: String,
}

impl Logger for VectorProcessLogger {
    type RecordType = VectorProcessLog;
    fn get_name(&self) -> &String {
        &self.name
    }
    fn get_buffer(&self) -> &EventBuffer<Self::RecordType> {
        &self.buffer
    }
    fn write_csv(self, dir: String) -> Result<(), Box<dyn std::error::Error>> {
        let file = std::fs::File::create(format!("{}/{}.csv", dir, self.name))?;
        let mut writer = csv::WriterBuilder::new()
            .has_headers(true)
            .from_writer(file);
        self.buffer.for_each(|log| {
            writer.serialize(log).expect("Failed to write log record to CSV file");
        });
        writer.flush()?;
        Ok(())
    }
}

impl VectorProcessLogger {
    pub fn new(name: String, capacity: usize) -> Self {
        VectorProcessLogger {
            buffer: EventBuffer::with_capacity(capacity),
            name,
        }
    }
}

#[derive(Clone, Debug)]
pub struct VectorProcessLog {
    pub time: String,
    pub element_name: String,
    pub element_type: String,
    pub process_data: VectorProcessLogType,
}

impl Serialize for VectorProcessLog {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("VectorProcessLog", 6)?;
        state.serialize_field("time", &self.time)?;
        state.serialize_field("element_name", &self.element_name)?;
        state.serialize_field("element_type", &self.element_type)?;
        let event_type: Option<&'static str>;
        let quantity: Option<f64>;
        let reason: Option<&'static str>;
        match &self.process_data {
            VectorProcessLogType::SourceSuccess { quantity: q } => {
                event_type = Some("SourceSuccess");
                quantity = Some(*q);
                reason = None;
            }
            VectorProcessLogType::SourceFailure { reason: r } => {
                event_type = Some("SourceFailure");
                quantity = None;
                reason = Some(r);
            }
            VectorProcessLogType::ProcessSuccess { quantity: q } => {
                event_type = Some("ProcessSuccess");
                quantity = Some(*q);
                reason = None;
            }
            VectorProcessLogType::ProcessFailure { reason: r } => {
                event_type = Some("ProcessFailure");
                quantity = None;
                reason = Some(r);
            }
            VectorProcessLogType::SinkSuccess { quantity: q } => {
                event_type = Some("SinkSuccess");
                quantity = Some(*q);
                reason = None;
            }
            VectorProcessLogType::SinkFailure { reason: r } => {
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
pub enum VectorProcessLogType  {
    SourceSuccess { quantity: f64 },
    SourceFailure { reason: &'static str },
    ProcessSuccess { quantity: f64 },
    ProcessFailure { reason: &'static str },
    SinkSuccess { quantity: f64 },
    SinkFailure { reason: &'static str },
}

define_source!(
    /// Source for the `VectorResource` type.
    name = VectorSource,
    resource_type = VectorResource,
    stock_state_type = VectorStockState,
    add_type = VectorResource,
    add_parameter_type = f64,
    create_method = |source: &mut Self, x: f64| -> VectorResource {
        let component_split = source.component_split.as_mut().unwrap_or_else(
            || panic!("Source component split not defined!")
        );
        let proportion = x / component_split.total();
        let mut new_resource = VectorResource { vec: [0.0; 5] };
        new_resource.vec.iter_mut().zip(component_split.vec.iter()).for_each(|(a, b)| *a = b * proportion);
        new_resource
    },
    check_update_method = |mut x: Self, time: MonotonicTime| {
        async move {
            let ds_state = x.req_downstream.send(()).await.next();
            match ds_state {
                Some(VectorStockState::Empty { .. } | VectorStockState::Normal { .. }) => {
                    let qty = x.create_quantity_dist.as_mut().unwrap_or_else(
                        || panic!("Source component split not defined!")
                    ).sample();
                    let new_resource = x.create(qty.clone());
                    x.push_downstream.send((new_resource.clone(), NotificationMetadata {
                        time,
                        element_from: x.element_name.clone(),
                        message: "New resource created".to_string(),
                    })).await;
                    x.log(time, VectorProcessLogType::SourceSuccess { quantity: qty }).await;
                },
                Some(VectorStockState::Full { .. }) => {
                    x.log(time, VectorProcessLogType::SourceFailure { reason: "Downstream is full" }).await;
                },
                None => {
                    x.log(time, VectorProcessLogType::SourceFailure { reason: "No downstream found" }).await;
                }
            };
            x
        }
    },
    fields = {
        component_split: Option<VectorResource>,
        create_quantity_dist: Option<Distribution>
    },
    log_record_type = VectorProcessLog,
    log_method = |x: &'a mut Self, time: MonotonicTime, details: VectorProcessLogType| {
        async move {
            let log = VectorProcessLog {
                time: time.to_chrono_date_time(0).unwrap().to_string(),
                element_name: x.element_name.clone(),
                element_type: x.element_type.clone(),
                process_data: details,
                
            };
            x.log_emitter.send(log).await;
        }
    },
    log_method_parameter_type = VectorProcessLogType
);

define_sink!(
    /// Sink for the `VectorResource` type.
    name = VectorSink,
    resource_type = VectorResource,
    stock_state_type = VectorStockState,
    subtract_type = VectorResource,
    subtract_parameters_type = f64,
    check_update_method = |mut sink: Self, time: MonotonicTime| {
        async move {
            let us_state = sink.req_upstream.send(()).await.next();
            match us_state {
                Some(VectorStockState::Full { .. } | VectorStockState::Normal { .. }) => {
                    let sink_qty = sink.destroy_quantity_dist.sample();
                    let removed = sink.withdraw_upstream.send((sink_qty, NotificationMetadata {
                        time,
                        element_from: sink.element_name.clone(),
                        message: "Resource removed".to_string(),
                    })).await.collect::<Vec<_>>();
                    sink.log(time, VectorProcessLogType::SinkSuccess { quantity: sink_qty }).await;
                },
                Some(VectorStockState::Empty { .. }) => {
                    sink.log(time, VectorProcessLogType::SinkFailure { reason: "Upstream is empty" }).await;
                },
                None => {
                    sink.log(time, VectorProcessLogType::SinkFailure { reason: "Upstream is not connected" }).await;
                }
            };
            sink
        }
    },
    fields = {
        destroy_quantity_dist: Distribution
    },
    log_record_type = VectorProcessLog,
    log_method = |x: &'a mut Self, time: MonotonicTime, details: VectorProcessLogType| {
        async move {
            // let state = x.get_state().await;
            let log = VectorProcessLog {
                time: time.to_chrono_date_time(0).unwrap().to_string(),
                element_name: x.element_name.clone(),
                element_type: x.element_type.clone(),
                process_data: details,
                
            };
            x.log_emitter.send(log).await;
        }
    },
    log_method_parameter_type = VectorProcessLogType
);

define_process!(
    /// Process for the `VectorResource` type.
    name = VectorProcess,
    stock_state_type = VectorStockState,
    resource_in_type = VectorResource,
    resource_in_parameter_type = f64,
    resource_out_type = VectorResource,
    resource_out_parameter_type = VectorProcess,
    check_update_method = |mut x: Self, time: MonotonicTime| {
        async move {

            let us_state = x.req_upstream.send(()).await.next();
            let ds_state = x.req_downstream.send(()).await.next();

            match (&us_state, &ds_state) {
                (
                    Some(VectorStockState::Normal {..}) | Some(VectorStockState::Full {..}),
                    Some(VectorStockState::Empty {..}) | Some(VectorStockState::Normal {..}),
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

                    x.log(time, VectorProcessLogType::ProcessSuccess { quantity: process_quantity }).await;
                },
                (Some(VectorStockState::Empty {..} ), _) => {
                    x.log(time, VectorProcessLogType::ProcessFailure { reason: "Upstream is empty" }).await;
                },
                (None, _) => {
                    x.log(time, VectorProcessLogType::ProcessFailure { reason: "Upstream is not connected" }).await;
                },
                (_, None) => {
                    x.log(time, VectorProcessLogType::ProcessFailure { reason: "Downstream is not connected" }).await;
                },
                (_, Some(VectorStockState::Full {..} )) => {
                    x.log(time, VectorProcessLogType::ProcessFailure { reason: "Downstream is full" }).await;
                },
            }
            x.time_to_next_event_counter = Some(Duration::from_secs_f64(x.process_duration_secs_dist.as_mut().unwrap_or_else(
                || panic!("Process duration distribution not set!")
            ).sample()));
            x
        }
    },
    fields = {
        process_quantity_dist: Option<Distribution>,
        process_duration_secs_dist: Option<Distribution>
    },
    log_record_type = VectorProcessLog,
    log_method = |x: &'a mut Self, time: MonotonicTime, details: VectorProcessLogType| {
        async move {
            // let state = x.get_state().await;
            let log = VectorProcessLog {
                time: time.to_chrono_date_time(0).unwrap().to_string(),
                element_name: x.element_name.clone(),
                element_type: x.element_type.clone(),
                process_data: details,
            };
            x.log_emitter.send(log).await;
        }
    },
    log_method_parameter_type = VectorProcessLogType
);

define_combiner_process!(
    /// Combiner process for the `VectorResource` type.
    name = VectorCombinerProcess,
    inflow_stock_state_types = (VectorStockState, VectorStockState),
    resource_in_types = (VectorResource, VectorResource),
    resource_in_parameter_types = (f64, f64),
    outflow_stock_state_type = VectorStockState,
    resource_out_type = VectorResource,
    resource_out_parameter_type = (),
    check_update_method = |mut x: Self, time: MonotonicTime| {
        async move {
            let us_states = (x.req_upstreams.0.send(()).await.next(), x.req_upstreams.1.send(()).await.next());
            let ds_state = x.req_downstream.send(()).await.next();

            match (&us_states.0, &us_states.1, &ds_state) {
                (
                    Some(VectorStockState::Normal { occupied: occupied_1, .. } ) | Some(VectorStockState::Full { occupied: occupied_1, .. } ),
                    Some(VectorStockState::Normal { occupied: occupied_2, .. } ) | Some(VectorStockState::Full { occupied: occupied_2, .. } ),
                    Some(VectorStockState::Empty { remaining_capacity, .. } ) | Some(VectorStockState::Normal { remaining_capacity, .. } ),
                ) => {
                    
                    let process_quantity = x.process_quantity_dist.as_mut().unwrap_or_else(
                        || panic!("Process quantity dist not defined!")
                    ).sample().min(*occupied_1 + *occupied_2).min(*remaining_capacity);

                    let qty1: VectorResource = x.withdraw_upstreams.0.send((process_quantity, NotificationMetadata {
                        time,
                        element_from: x.element_name.clone(),
                        message: "Withdrawing item".into(),
                    })).await.next().unwrap();

                    let qty2: VectorResource = x.withdraw_upstreams.1.send((process_quantity, NotificationMetadata {
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
                    x.log(time, VectorProcessLogType::ProcessSuccess { quantity: process_quantity }).await;
                },
                (_, _, Some(VectorStockState::Full {..} )) => {
                    x.log(time, VectorProcessLogType::ProcessFailure { reason: "Downstream is full" }).await;
                },
                (_, _, None) => {
                    x.log(time, VectorProcessLogType::ProcessFailure { reason: "Downstream is not connected" }).await;
                },
                (None, _, _) => {
                    x.log(time, VectorProcessLogType::ProcessFailure { reason: "Upstream 0 is not connected" }).await;
                }
                (_, None, _) => {
                    x.log(time, VectorProcessLogType::ProcessFailure { reason: "Upstream 1 is not connected" }).await;
                },
                (Some(VectorStockState::Empty {..} ), _, _) => {
                    x.log(time, VectorProcessLogType::ProcessFailure { reason: "Upstream 0 is empty" }).await;
                }
                (_, Some(VectorStockState::Empty {..} ), _) => {
                    x.log(time, VectorProcessLogType::ProcessFailure { reason: "Upstream 1 is empty" }).await;
                }
            };
            x.time_to_next_event_counter = Some(Duration::from_secs_f64(x.process_duration_secs_dist.as_mut().unwrap_or_else(
                || panic!("Process duration distribution not set!")
            ).sample()));
            x
        }
    },
    fields = {
        process_quantity_dist: Option<Distribution>,
        process_duration_secs_dist: Option<Distribution>
    },
    log_record_type = VectorProcessLog,
    log_method = |x: &'a mut Self, time: MonotonicTime, details: VectorProcessLogType| {
        async move {
            // let state = x.get_state().await;
            let log = VectorProcessLog {
                time: time.to_chrono_date_time(0).unwrap().to_string(),
                element_name: x.element_name.clone(),
                element_type: x.element_type.clone(),
                process_data: details,
                
            };
            x.log_emitter.send(log).await;
        }
    },
    log_method_parameter_type = VectorProcessLogType
);

define_splitter_process!(
    /// Splitter process for the `VectorResource` type.
    name = VectorSplitterProcess,
    inflow_stock_state_type = VectorStockState,
    resource_in_type = VectorResource,
    resource_in_parameter_type = f64,
    outflow_stock_state_types = (VectorStockState, VectorStockState),
    resource_out_types = (VectorResource, VectorResource),
    resource_out_parameter_types = (VectorResource, VectorResource),
    check_update_method = |mut x: Self, time: MonotonicTime| {
        async move {
            let us_state = x.req_upstream.send(()).await.next();
            let ds_states = (x.req_downstreams.0.send(()).await.next(), x.req_downstreams.1.send(()).await.next());

            match (&us_state, &ds_states.0, &ds_states.1) {
                (
                    Some(VectorStockState::Normal { occupied, .. } ) | Some(VectorStockState::Full { occupied, .. } ),
                    Some(VectorStockState::Empty { remaining_capacity: remaining_capacity_1, .. } ) | Some(VectorStockState::Normal { remaining_capacity: remaining_capacity_1, .. } ),
                    Some(VectorStockState::Empty { remaining_capacity: remaining_capacity_2, .. } ) | Some(VectorStockState::Normal { remaining_capacity: remaining_capacity_2, .. } ),
                ) => {

                    let process_quantity = x.process_quantity_dist.as_mut().unwrap_or_else(
                        || panic!("Process quantity dist not defined!")
                    ).sample().min(*occupied).min(*remaining_capacity_1 + *remaining_capacity_2);

                    let processed_resource: VectorResource = x.withdraw_upstream.send((process_quantity, NotificationMetadata {
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

                    x.log(time, VectorProcessLogType::ProcessSuccess { quantity: process_quantity }).await;
                },
                (Some(VectorStockState::Empty {..} ), _, _) => {
                    x.log(time, VectorProcessLogType::ProcessFailure { reason: "Upstream is empty" }).await;
                },
                (None, _, _) => {
                    x.log(time, VectorProcessLogType::ProcessFailure { reason: "Upstream is not connected" }).await;
                },
                (_, None, _) => {
                    x.log(time, VectorProcessLogType::ProcessFailure { reason: "Downstream 0 is not connected" }).await;
                },
                (_, _, None) => {
                    x.log(time, VectorProcessLogType::ProcessFailure { reason: "Downstream 1 is not connected" }).await;
                },
                (_, Some(VectorStockState::Full {..} ), _) => {
                    x.log(time, VectorProcessLogType::ProcessFailure { reason: "Downstream 0 is full" }).await;
                },
                (_, _, Some(VectorStockState::Full {..} )) => {
                    x.log(time, VectorProcessLogType::ProcessFailure { reason: "Downstream 1 is full" }).await;
                },
            };
            x.time_to_next_event_counter = Some(Duration::from_secs_f64(x.process_duration_secs_dist.as_mut().unwrap_or_else(
                || panic!("Process duration distribution not set!")
            ).sample()));
            x
        }
    },
    fields = {
        process_quantity_dist: Option<Distribution>,
        process_duration_secs_dist: Option<Distribution>
    },
    log_record_type = VectorProcessLog,
    log_method = |x: &'a mut Self, time: MonotonicTime, details: VectorProcessLogType| {
        async move {
            // let state = x.get_state().await;
            let log = VectorProcessLog {
                time: time.to_chrono_date_time(0).unwrap().to_string(),
                element_name: x.element_name.clone(),
                element_type: x.element_type.clone(),
                process_data: details,
                
            };
            x.log_emitter.send(log).await;
        }
    },
    log_method_parameter_type = VectorProcessLogType
);
