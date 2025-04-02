use nexosim::{model::Context, ports::Output, time::MonotonicTime};
use crate::{common::{Distribution, DistributionFactory, EventLog, EventLogger, NotificationMetadata}, core::{Mailbox, ResourceAdd, ResourceRemove, SimInit, StateEq}, define_combiner_process, define_process, define_sink, define_source, define_stock};

#[derive(Debug, Clone)]
pub enum QueueState {
    Empty {
        occupied: i32,
        empty: i32
    },
    Normal {
        occupied: i32,
        empty: i32
    },
    Full {
        occupied: i32,
        empty: i32
    }
}

impl StateEq for QueueState {
    fn is_same_state(&self, other: &Self) -> bool {
        match (self, other) {
            (QueueState::Empty { occupied: _, empty: _ }, QueueState::Empty { occupied: _, empty: _ }) => {
                true
            },
            (QueueState::Normal { occupied: _, empty: _ }, QueueState::Normal { occupied: _, empty: _ }) => {
                true
            },
            (QueueState::Full { occupied: _, empty: _ }, QueueState::Full { occupied: _, empty: _ }) => {
                true
            },
            _ => false
        }
    }
}

#[derive(Debug, Clone)]
pub struct QueueVector {
    pub queue: Vec<i32>,
}

impl ResourceAdd<Vec<i32>> for QueueVector {
    fn add(&mut self, other: Vec<i32>) {
        self.queue.extend(other);
    }
}

impl ResourceRemove<i32, Vec<i32>> for QueueVector {
    fn sub(&mut self, other: i32) -> Vec<i32> {
        let mut removed_items = vec![];
        for _ in 0..other {
            if let Some(item) = self.queue.pop() {
                removed_items.push(item);
            } else {
                break;
            }
        }
        removed_items
    }
}


define_stock!(
    name = MyQueueStock,
    resource_type = QueueVector,
    initial_resource = QueueVector { queue: vec![] },
    add_type = Vec<i32>,
    remove_type = Vec<i32>,
    remove_parameter_type = i32,
    state_type = QueueState,
    fields = {
        low_capacity: i32,
        max_capacity: i32
    },
    get_state_method = |x: &MyQueueStock| -> QueueState {
        let occupied = x.resource.queue.len() as i32;
        let empty = (x.max_capacity - occupied).max(0);
        if occupied <= x.low_capacity {
            return QueueState::Empty {
                occupied,
                empty,
            }
        } else if occupied >= x.max_capacity {
            return QueueState::Full {
                occupied,
                empty,
            }
        } else {
            return QueueState::Normal {
                occupied,
                empty,
            }
        }
    },
    check_update_method = |x: &mut MyQueueStock, cx: &mut Context<MyQueueStock>| {
    }
);


define_source!(
    name = MyQueueSource,
    resource_type = Vec<i32>,
    stock_state_type = QueueState,
    add_type = Vec<i32>,
    add_parameter_type = i32,
    create_method = |mut source: &mut Self, x: i32| -> Vec<i32> {
        let mut resource = source.resource.clone();
        resource.push(x);
        resource
    },
    check_update_method = |mut x: Self, time: MonotonicTime| {
        async move {
            let ds_state = x.req_downstream.send(()).await.next();
            match ds_state {
                Some(QueueState::Empty {..}) | Some(QueueState::Normal {..}) => {
                    let new_resources = x.create(x.next_id);
                    x.next_id += 1;
                    x.push_downstream.send((new_resources.clone(), NotificationMetadata {
                        time: time.clone(),
                        element_from: x.element_name.clone(),
                        message: "New item".to_string(),
                    })).await;
                    x.log_emitter.send(EventLog {
                        time: time.clone(),
                        element_name: x.element_name.clone(),
                        element_type: x.element_type.clone(),
                        log_type: "info".into(),
                        json_data: format!("{{\"message\": \"Created new to queue\", \"item\": {:?}}}", new_resources),
                    }).await;
                },
                Some(QueueState::Full {..}) => {
                    // Do nothing
                    x.log_emitter.send(EventLog {
                        time: time.clone(),
                        element_name: x.element_name.clone(),
                        element_type: x.element_type.clone(),
                        log_type: "info".into(),
                        json_data: format!("{{\"message\": \"Failed to create new item as downstream stock is full\"}}"),
                    }).await;
                },
                None => {
                    // Do nothing
                    x.log_emitter.send(EventLog {
                        time: time.clone(),
                        element_name: x.element_name.clone(),
                        element_type: x.element_type.clone(),
                        log_type: "info".into(),
                        json_data: format!("{{\"message\": \"Failed to create new item as no downstream stock is connected\"}}"),
                    }).await;
                },
            }
            x
        }
    },
    fields = {
        next_id: i32
    }
);


define_sink!(
    name = MyQueueSink,
    resource_type = Vec<i32>,
    stock_state_type = QueueState,
    subtract_type = Vec<i32>,
    subtract_parameters_type = i32,
    destroy_method = |sink: &mut MyQueueSink, x: i32| -> Vec<i32> {
        let mut resource = sink.resource.clone();
        resource.retain(|&y| y != x);
        resource
    },
    check_update_method = |mut sink: Self, time: MonotonicTime| {
        async move {
            let us_state = sink.req_upstream.send(()).await.next();

            match us_state {
                Some(QueueState::Normal {..}) | Some(QueueState::Full {..}) => {
                    let sink_quantity = sink.sink_quantity_dist.sample().round() as i32;
                    let item = sink.withdraw_upstream.send((sink_quantity, NotificationMetadata {
                        time,
                        element_from: sink.element_name.clone(),
                        message: "Withdrawing item".into(),
                    })).await.next().unwrap();
                    sink.log_emitter.send(EventLog {
                        time,
                        element_name: sink.element_name.clone(),
                        element_type: sink.element_type.clone(),
                        log_type: "info".into(),
                        json_data: format!("{{\"message\": \"Received item\", \"item\": {:?}}}", item),
                    }).await;
                    sink.log(time, "Destroy".into(), format!("{:?}", item)).await;
                },
                Some(QueueState::Empty {..}) => {
                    // Do nothing
                    sink.log_emitter.send(EventLog {
                        time,
                        element_name: sink.element_name.clone(),
                        element_type: sink.element_type.clone(),
                        log_type: "info".into(),
                        json_data: format!("{{\"message\": \"Failed to receive item as upstream stock is empty\"}}"),
                    }).await;
                },
                None => {
                    // Do nothing
                    sink.log_emitter.send(EventLog {
                        time,
                        element_name: sink.element_name.clone(),
                        element_type: sink.element_type.clone(),
                        log_type: "info".into(),
                        json_data: format!("{{\"message\": \"Failed to receive item as no upstream stock is connected\"}}"),
                    }).await;
                },
            };
            sink
        }
    },
    fields = {
        next_id: i32,
        sink_quantity_dist: Distribution
    },
);


define_process!(
    name = MyQueueProcess,

    stock_state_type = QueueState,
    resource_in_type = Vec<i32>,
    resource_in_parameter_type = i32,
    resource_out_type = Vec<i32>,
    resource_out_parameter_type = i32,

    check_update_method = |mut x: Self, time: MonotonicTime| {
        async move {
            let us_state = x.req_upstream.send(()).await.next();
            let ds_state = x.req_downstream.send(()).await.next();

            match (&us_state, &ds_state) {
                (
                    Some(QueueState::Normal {..} ) | Some(QueueState::Full {..} ),
                    Some(QueueState::Empty {..} ) | Some(QueueState::Normal {..} ),
                ) => {
                    let sink_quantity = x.process_quantity_dist.sample().round() as i32;
                    
                    let items = x.withdraw_upstream.send((sink_quantity, NotificationMetadata {
                        time,
                        element_from: x.element_name.clone(),
                        message: "Withdrawing item".into(),
                    })).await.next().unwrap();

                    x.push_downstream.send((items.clone(), NotificationMetadata {
                        time,
                        element_from: x.element_name.clone(),
                        message: "Processing complete".into(),
                    })).await;

                    x.log_emitter.send(EventLog {
                        time,
                        element_name: x.element_name.clone(),
                        element_type: x.element_type.clone(),
                        log_type: "info".into(),
                        json_data: format!("{{\"message\": \"Processed item\", \"item\": {:?}}}", items),
                    }).await;
                },
                (
                    Some(QueueState::Empty {..} ) | None,
                    _
                ) => {
                    // Do nothing
                    x.log_emitter.send(EventLog {
                        time,
                        element_name: x.element_name.clone(),
                        element_type: x.element_type.clone(),
                        log_type: "info".into(),
                        json_data: format!("{{\"message\": \"Failed to receive item as upstream stock is empty or isn't connected\"}}"),
                    }).await;
                },
                (
                    _,
                    Some(QueueState::Full {..} ) | None,
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
            }
            x
        }
    },
    fields = {
        process_quantity_dist: Distribution
    },
);

define_combiner_process!(
    name = MyQueueCombinerProcess,
    stock_state_type = QueueState,
    resource_in_types = (Vec<i32>, Vec<i32>),
    resource_in_parameter_types = (i32, i32),
    resource_out_type = Vec<i32>,
    resource_out_parameter_type = i32,
    check_update_method = |mut x: Self, time: MonotonicTime| {
        async move {
            let us_states = (x.req_upstreams.0.send(()).await.next(), x.req_upstreams.1.send(()).await.next());
            let ds_state = x.req_downstream.send(()).await.next();

            match (&us_states.0, &us_states.1, &ds_state) {
                (
                    Some(QueueState::Normal { occupied: occupied0, .. } ) | Some(QueueState::Full { occupied: occupied0, .. } ),
                    Some(QueueState::Normal { occupied: occupied1, .. } ) | Some(QueueState::Full { occupied: occupied1, .. } ),
                    Some(QueueState::Empty { empty, .. } ) | Some(QueueState::Normal { empty, .. } ),
                ) => {
                    let sink_quantity = (x.process_quantity_dist.sample().round() as i32).min(*occupied0).min(*occupied1).max(*empty);
                    
                    let items0 = x.withdraw_upstreams.0.send((sink_quantity, NotificationMetadata {
                        time,
                        element_from: x.element_name.clone(),
                        message: "Withdrawing item".into(),
                    })).await.next().unwrap();

                    let items1 = x.withdraw_upstreams.1.send((sink_quantity, NotificationMetadata {
                        time,
                        element_from: x.element_name.clone(),
                        message: "Withdrawing item".into(),
                    })).await.next().unwrap();

                    let items = items0.into_iter().chain(items1.into_iter()).collect::<Vec<i32>>();

                    x.push_downstream.send((items.clone(), NotificationMetadata {
                        time,
                        element_from: x.element_name.clone(),
                        message: "Processing complete".into(),
                    })).await;

                    x.log_emitter.send(EventLog {
                        time,
                        element_name: x.element_name.clone(),
                        element_type: x.element_type.clone(),
                        log_type: "info".into(),
                        json_data: format!("{{\"message\": \"Processed item\", \"item\": {:?}}}", items),
                    }).await;

                },
                (
                    _, _, Some(QueueState::Full {..} ) | None,
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
                    Some(QueueState::Empty {..} ) | None, _, _
                ) | (
                    _, Some(QueueState::Empty {..} ) | None, _
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