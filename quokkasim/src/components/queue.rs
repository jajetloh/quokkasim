use nexosim::{model::Context, ports::Output, time::MonotonicTime};
use serde::{ser::SerializeStruct, Serialize};
use crate::{common::{Distribution, EventLogger}, core::{ResourceAdd, ResourceRemove, StateEq}, define_combiner_process, define_process, define_sink, define_source, define_stock};

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

#[derive(Serialize, Debug, Clone)]
pub struct QueueStockLog {
    pub time: String,
    pub element_name: String,
    pub element_type: String,
    pub log_type: String,
    pub occupied: i32,
    pub empty: i32,
    pub state: String,
    pub contents: String,
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
        
    },
    log_record_type = QueueStockLog,
    log_method = |x: &'a mut Self, time: MonotonicTime, log_type: String| {
        async move {
            let state = x.get_state().await;
            let log = QueueStockLog {
                time: time.to_chrono_date_time(0).unwrap().to_string(),
                element_name: x.element_name.clone(),
                element_type: x.element_type.clone(),
                log_type,
                empty: match state {
                    QueueState::Empty { empty, .. } => empty,
                    QueueState::Normal { empty, .. } => empty,
                    QueueState::Full { empty, .. } => empty,
                },
                occupied: match state {
                    QueueState::Empty { occupied, .. } => occupied,
                    QueueState::Normal { occupied, .. } => occupied,
                    QueueState::Full { occupied, .. } => occupied,
                },
                state: match state {
                    QueueState::Empty {..} => "Empty".to_string(),
                    QueueState::Normal {..} => "Normal".to_string(),
                    QueueState::Full {..} => "Full".to_string(),
                },
                contents: x.resource.queue.iter().map(|z| z.to_string()).collect::<Vec<String>>().join(" "),
            };
            x.log_emitter.send(log).await;
        }
    }
);

#[derive(Clone, Debug)]
pub struct QueueProcessLog {
    pub time: String,
    pub element_name: String,
    pub element_type: String,
    pub process_data: QueueProcessLogType,
}

impl Serialize for QueueProcessLog {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("QueueProcessLog", 6)?;
        state.serialize_field("time", &self.time)?;
        state.serialize_field("element_name", &self.element_name)?;
        state.serialize_field("element_type", &self.element_type)?;
        let event_type: Option<&'static str>;
        let quantity: Option<i32>;
        let reason: Option<&'static str>;
        match &self.process_data {
            QueueProcessLogType::SourceSuccess { quantity: q } => {
                event_type = Some("SourceSuccess");
                quantity = Some(*q);
                reason = None;
            }
            QueueProcessLogType::SourceFailure { reason: r } => {
                event_type = Some("SourceFailure");
                quantity = None;
                reason = Some(r);
            }
            QueueProcessLogType::ProcessSuccess { quantity: q } => {
                event_type = Some("ProcessSuccess");
                quantity = Some(*q);
                reason = None;
            }
            QueueProcessLogType::ProcessFailure { reason: r } => {
                event_type = Some("ProcessFailure");
                quantity = None;
                reason = Some(r);
            }
            QueueProcessLogType::SinkSuccess { quantity: q } => {
                event_type = Some("SinkSuccess");
                quantity = Some(*q);
                reason = None;
            }
            QueueProcessLogType::SinkFailure { reason: r } => {
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
pub enum QueueProcessLogType  {
    SourceSuccess { quantity: i32 },
    SourceFailure { reason: &'static str },
    ProcessSuccess { quantity: i32 },
    ProcessFailure { reason: &'static str },
    SinkSuccess { quantity: i32 },
    SinkFailure { reason: &'static str },
}


define_source!(
    /// Source for the `Vec<i32>` type.
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
                    x.log(time, QueueProcessLogType::SourceSuccess { quantity: 1 }).await;
                },
                Some(QueueState::Full {..}) => {
                    // Do nothing
                    x.log(time, QueueProcessLogType::SourceFailure { reason: "Downstream is full" }).await;
                },
                None => {
                    x.log(time, QueueProcessLogType::SourceFailure { reason: "Downstream is not connected" }).await;
                },
            }
            x
        }
    },
    fields = {
        next_id: i32
    },
    log_record_type = QueueProcessLog,
    log_method = |x: &'a mut Self, time: MonotonicTime, details: QueueProcessLogType| {
        async move {
            let log = QueueProcessLog {
                time: time.to_chrono_date_time(0).unwrap().to_string(),
                element_name: x.element_name.clone(),
                element_type: x.element_type.clone(),
                process_data: details,
                
            };
            x.log_emitter.send(log).await;
        }
    },
    log_method_parameter_type = QueueProcessLogType
);


define_sink!(
    /// Sink for the `Vec<i32>` type.
    name = MyQueueSink,
    resource_type = Vec<i32>,
    stock_state_type = QueueState,
    subtract_type = Vec<i32>,
    subtract_parameters_type = i32,
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
                    sink.log(time, QueueProcessLogType::SinkSuccess { quantity: sink_quantity }).await;
                },
                Some(QueueState::Empty {..}) => {
                    sink.log(time, QueueProcessLogType::SinkFailure { reason: "Upstream is empty" }).await;
                },
                None => {
                    sink.log(time, QueueProcessLogType::SinkFailure { reason: "Upstream is not connected" }).await;
                },
            };
            sink
        }
    },
    fields = {
        next_id: i32,
        sink_quantity_dist: Distribution
    },
    log_record_type = QueueProcessLog,
    log_method = |x: &'a mut Self, time: MonotonicTime, details: QueueProcessLogType| {
        async move {
            // let state = x.get_state().await;
            let log = QueueProcessLog {
                time: time.to_chrono_date_time(0).unwrap().to_string(),
                element_name: x.element_name.clone(),
                element_type: x.element_type.clone(),
                process_data: details,
                
            };
            x.log_emitter.send(log).await;
        }
    },
    log_method_parameter_type = QueueProcessLogType
);


define_process!(
    /// Process for the `Vec<i32>` type.
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
                    Some(QueueState::Normal { occupied, .. } ) | Some(QueueState::Full { occupied, .. } ),
                    Some(QueueState::Empty { empty, .. } ) | Some(QueueState::Normal { empty, .. } ),
                ) => {
                    let process_quantity = (x.process_quantity_dist.as_mut().unwrap_or_else(
                        || panic!("Process quantity dist not defined!")
                    ).sample().round() as i32).min(*occupied).min(*empty);

                    let items = x.withdraw_upstream.send((process_quantity, NotificationMetadata {
                        time,
                        element_from: x.element_name.clone(),
                        message: "Withdrawing item".into(),
                    })).await.next().unwrap();

                    x.push_downstream.send((items.clone(), NotificationMetadata {
                        time,
                        element_from: x.element_name.clone(),
                        message: "Processing complete".into(),
                    })).await;

                    x.log(time, QueueProcessLogType::ProcessSuccess { quantity: process_quantity }).await;
                },
                (Some(QueueState::Empty {..} ), _) => {
                    x.log(time, QueueProcessLogType::ProcessFailure { reason: "Upstream is empty" }).await;
                },
                (None, _) => {
                    x.log(time, QueueProcessLogType::ProcessFailure { reason: "Upstream is empty" }).await;
                },
                (_, Some(QueueState::Full {..} )) => {
                    x.log(time, QueueProcessLogType::ProcessFailure { reason: "Downstream is full" }).await;
                },
                (_, None) => {
                    x.log(time, QueueProcessLogType::ProcessFailure { reason: "Downstream is not connected" }).await;
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
    log_record_type = QueueProcessLog,
    log_method = |x: &'a mut Self, time: MonotonicTime, details: QueueProcessLogType| {
        async move {
            let log = QueueProcessLog {
                time: time.to_chrono_date_time(0).unwrap().to_string(),
                element_name: x.element_name.clone(),
                element_type: x.element_type.clone(),
                process_data: details,
                
            };
            x.log_emitter.send(log).await;
        }
    },
    log_method_parameter_type = QueueProcessLogType
);

define_combiner_process!(
    /// Combiner for the `Vec<i32>` type.
    name = MyQueueCombinerProcess,
    inflow_stock_state_types = (QueueState, QueueState),
    resource_in_types = (Vec<i32>, Vec<i32>),
    resource_in_parameter_types = (i32, i32),
    outflow_stock_state_type = QueueState,
    resource_out_type = Vec<i32>,
    resource_out_parameter_type = (),
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
                    let process_quantity = (x.process_quantity_dist.as_mut().unwrap_or_else(
                        || panic!("Process quantity dist not defined!")
                    ).sample().round() as i32).min(*occupied0 + *occupied1).min(*empty);

                    let items0 = x.withdraw_upstreams.0.send((process_quantity, NotificationMetadata {
                        time,
                        element_from: x.element_name.clone(),
                        message: "Withdrawing item".into(),
                    })).await.next().unwrap();

                    let items1 = x.withdraw_upstreams.1.send((process_quantity, NotificationMetadata {
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

                    x.log(time, QueueProcessLogType::ProcessSuccess { quantity: process_quantity }).await;
                },
                (_, _, Some(QueueState::Full {..} )) => {
                    x.log(time, QueueProcessLogType::ProcessFailure { reason: "Downstream is full" }).await;
                },
                (_, _, None) => {
                    x.log(time, QueueProcessLogType::ProcessFailure { reason: "Downstream is not connected" }).await;
                },
                (Some(QueueState::Empty {..} ), _, _) => {
                    x.log(time, QueueProcessLogType::ProcessFailure { reason: "Upstream 0 is empty" }).await;
                },
                (None, _, _) => {
                    x.log(time, QueueProcessLogType::ProcessFailure { reason: "Upstream 0 is not connected" }).await;
                },
                (_, Some(QueueState::Empty {..} ), _) => {
                    x.log(time, QueueProcessLogType::ProcessFailure { reason: "Upstream 1 is empty" }).await;
                },
                (_, None, _) => {
                    x.log(time, QueueProcessLogType::ProcessFailure { reason: "Upstream 1 is not connected" }).await;
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
    log_record_type = QueueProcessLog,
    log_method = |x: &'a mut Self, time: MonotonicTime, details: QueueProcessLogType| {
        async move {
            let log = QueueProcessLog {
                time: time.to_chrono_date_time(0).unwrap().to_string(),
                element_name: x.element_name.clone(),
                element_type: x.element_type.clone(),
                process_data: details,
                
            };
            x.log_emitter.send(log).await;
        }
    },
    log_method_parameter_type = QueueProcessLogType
);