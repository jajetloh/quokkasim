use nexosim::{model::Context, ports::Output, time::MonotonicTime};
use crate::{common::{Distribution, EventLog, EventLogger, NotificationMetadata}, core::{Mailbox, ResourceAdd, ResourceRemove, SimInit, StateEq}, define_combiner_process, define_process, define_sink, define_source, define_stock};

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
        self.vec.iter_mut().zip(item.vec.iter()).for_each(|(a, b)| *a += b);
    }
}

impl ResourceRemove<f64, ArrayResource> for ArrayResource {
    fn sub(&mut self, qty: f64) -> ArrayResource {
        // Removes proportionally from each element of the array
        let proportion = qty / self.total();
        let removed = self.vec.map(|x| x * proportion);
        self.vec.iter_mut().zip(removed.iter()).for_each(|(a, b)| *a -= b);
        ArrayResource { vec: removed }
    }
}

impl ResourceRemove<ArrayResource, ArrayResource> for ArrayResource {
    fn sub(&mut self, qty: ArrayResource) -> ArrayResource {
        self.vec.iter_mut().zip(qty.vec.iter()).for_each(|(a, b)| *a -= b);
        qty
    }
}

define_stock!(
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
    }
);

define_source!(
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
                    let new_resource = x.create(qty);
                    x.push_downstream.send((new_resource.clone(), NotificationMetadata {
                        time,
                        element_from: x.element_name.clone(),
                        message: "New resource created".to_string(),
                    })).await;
                    x.log_emitter.send(EventLog {
                        time,
                        element_name: x.element_name.clone(),
                        element_type: "ArraySource".to_string(),
                        log_type: "New resource created".to_string(),
                        json_data: format!(
                            "{{\"new_resource\": {:?}}}",
                            new_resource
                        ),
                    }).await;
                },
                Some(ArrayStockState::Full { .. }) => {
                    x.log_emitter.send(EventLog {
                        time,
                        element_name: x.element_name.clone(),
                        element_type: "ArraySource".to_string(),
                        log_type: "Stock is full".to_string(),
                        json_data: format!(
                            "{{\"message\": \"Stock is full\"}}"
                        ),
                    }).await;
                },
                None => {
                    x.log_emitter.send(EventLog {
                        time,
                        element_name: x.element_name.clone(),
                        element_type: "ArraySource".to_string(),
                        log_type: "No downstream state".to_string(),
                        json_data: format!(
                            "{{\"message\": \"No downstream state\"}}"
                        ),
                    }).await;
                }
            };
            x
        }
    },
    fields = {
        component_split: ArrayResource,
        create_quantity_dist: Distribution
    }
);

define_sink!(
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
                    // let ved = sink.destroy(sink.destroy_quantity_dist.sample());
                    // let removed = sink.resource.sub(sink.destroy_quantity_dist.sample());
                    let sink_qty = sink.destroy_quantity_dist.sample();
                    let removed = sink.withdraw_upstream.send((sink_qty, NotificationMetadata {
                        time,
                        element_from: sink.element_name.clone(),
                        message: "Resource removed".to_string(),
                    })).await.collect::<Vec<_>>();
                    sink.log_emitter.send(EventLog {
                        time,
                        element_name: sink.element_name.clone(),
                        element_type: "ArraySink".to_string(),
                        log_type: "Resource destroyed".to_string(),
                        json_data: format!(
                            "{{\"removed\": {:?}}}",
                            removed
                        ),
                    }).await;
                },
                Some(ArrayStockState::Empty { .. }) => {
                    sink.log_emitter.send(EventLog {
                        time,
                        element_name: sink.element_name.clone(),
                        element_type: "ArraySink".to_string(),
                        log_type: "Stock is full".to_string(),
                        json_data: format!(
                            "{{\"message\": \"Stock is empty\"}}"
                        ),
                    }).await;
                },
                None => {
                    sink.log_emitter.send(EventLog {
                        time,
                        element_name: sink.element_name.clone(),
                        element_type: "ArraySink".to_string(),
                        log_type: "No upstream state".to_string(),
                        json_data: format!(
                            "{{\"message\": \"No upstream state\"}}"
                        ),
                    }).await;
                }
            };
            sink
        }
    },
    fields = {
        destroy_quantity_dist: Distribution
    },
);

define_process!(
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
                    let process_quantity = x.process_quantity_dist.sample();
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

                    x.log_emitter.send(EventLog {
                        time,
                        element_name: x.element_name.clone(),
                        element_type: x.element_type.clone(),
                        log_type: "info".into(),
                        json_data: format!("Processed quantity {:?} ({:?})", process_quantity, moved),
                    }).await;
                },
                (
                    Some(ArrayStockState::Empty {..} ) | None,
                    _
                ) => {
                    // Do nothing
                    x.log_emitter.send(EventLog {
                        time,
                        element_name: x.element_name.clone(),
                        element_type: x.element_type.clone(),
                        log_type: "info".into(),
                        json_data: format!("{{\"message\": \"Failed to receive item as downstream stock is full or isn't connected\"}}"),
                    }).await;
                },
                _ => {
                    // Do nothing
                    x.log_emitter.send(EventLog {
                        time,
                        element_name: x.element_name.clone(),
                        element_type: x.element_type.clone(),
                        log_type: "info".into(),
                        json_data: format!("{{\"message\": \"Failed to receive item as downstream stock is full or isn't connected\"}}"),
                    }).await;
                }
            }
            x
        }
    },
    fields = {
        process_quantity_dist: Distribution
    },
);

define_combiner_process!(
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
                    Some(ArrayStockState::Normal { .. } ) | Some(ArrayStockState::Full { .. } ),
                    Some(ArrayStockState::Normal { .. } ) | Some(ArrayStockState::Full { .. } ),
                    Some(ArrayStockState::Empty { .. } ) | Some(ArrayStockState::Normal { .. } ),
                ) => {

                    let process_qty = x.process_quantity_dist.sample();
                    
                    let qty1: ArrayResource = x.withdraw_upstreams.0.send((process_qty, NotificationMetadata {
                        time,
                        element_from: x.element_name.clone(),
                        message: "Withdrawing item".into(),
                    })).await.next().unwrap();

                    let qty2: ArrayResource = x.withdraw_upstreams.1.send((process_qty, NotificationMetadata {
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

                    x.log_emitter.send(EventLog {
                        time,
                        element_name: x.element_name.clone(),
                        element_type: x.element_type.clone(),
                        log_type: "info".into(),
                        json_data: format!("{{\"message\": \"Processed quantity\", \"quantity\": {:?}}}", total),
                    }).await;

                },
                (
                    _, _, Some(ArrayStockState::Full {..} ) | None,
                ) => {
                    // Do nothing
                    x.log_emitter.send(EventLog {
                        time,
                        element_name: x.element_name.clone(),
                        element_type: x.element_type.clone(),
                        log_type: "info".into(),
                        json_data: format!("{{\"message\": \"Failed to receive item as downstream stock is full or isn't connected\"}}"),
                    }).await;
                },
                (
                    Some(ArrayStockState::Empty {..} ) | None, _, _
                ) | (
                    _, Some(ArrayStockState::Empty {..} ) | None, _
                ) => {
                    // Do nothing
                    x.log_emitter.send(EventLog {
                        time,
                        element_name: x.element_name.clone(),
                        element_type: x.element_type.clone(),
                        log_type: "info".into(),
                        json_data: format!("{{\"message\": \"Failed to receive item as upstream stocks are empty or aren't connected\"}}"),
                    }).await;
                }
            };
            x
        }
    },
    fields = {
        process_quantity_dist: Distribution
    },
);