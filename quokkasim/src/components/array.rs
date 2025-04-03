use nexosim::model::Context;
use tai_time::MonotonicTime;

use crate::{core::{Distribution, EventLog, NotificationMetadata, ResourceAdd, ResourceRemove, StateEq}, define_sink, define_source, define_stock};

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

impl ResourceAdd<[f64; 5]> for ArrayResource {
    fn add(&mut self, item: [f64; 5]) {
        self.vec.iter_mut().zip(item.iter()).for_each(|(a, b)| *a += b);
    }
}

impl ResourceRemove<f64, [f64; 5]> for ArrayResource {
    fn sub(&mut self, qty: f64) -> [f64; 5] {
        // Removes proportionally from each element of the array
        let proportion = qty / self.total();
        let removed = self.vec.map(|x| x * proportion);
        self.vec.iter_mut().zip(removed.iter()).for_each(|(a, b)| *a -= b);
        removed
    }
}

impl ResourceRemove<[f64; 5], [f64; 5]> for ArrayResource {
    fn sub(&mut self, qty: [f64; 5]) -> [f64; 5] {
        self.vec.iter_mut().zip(qty.iter()).for_each(|(a, b)| *a -= b);
        qty
    }
}

define_stock!(
    name = ArrayStock,
    resource_type = ArrayResource,
    initial_resource = ArrayResource { vec: [0.0; 5] },
    add_type = [f64; 5],
    remove_type = [f64; 5],
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
                    let new_resource = x.create(x.create_quantity_dist.sample());
                    x.push_downstream.send((new_resource, NotificationMetadata {
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
    destroy_method = |mut x: Self, qty: f64|{
    },
    check_update_method = |mut sink: Self, time: MonotonicTime| {
        async move {
            let us_state = sink.req_upstream.send(()).await.next();
            match us_state {
                Some(ArrayStockState::Full { .. } | ArrayStockState::Normal { .. }) => {
                    // let removed = sink.destroy(sink.destroy_quantity_dist.sample());
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
        }
    },
    fields = {
        destroy_quantity_dist: Distribution
    },
);
