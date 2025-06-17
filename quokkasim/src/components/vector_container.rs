use std::{collections::VecDeque, time::Duration};

use serde::{ser::SerializeStruct, Serialize};

use crate::prelude::*;

#[derive(Debug, Clone, Default)]
pub struct F64Container {
    pub id: String,
    pub resource: Option<f64>,
    pub capacity: f64,
}

impl ResourceAdd<f64> for F64Container {
    fn add(&mut self, arg: f64) {
        if let Some(resource) = &mut self.resource {
            *resource += arg;
        } else {
            self.resource = Some(arg);
        }
    }
}

impl ResourceRemoveAll<f64> for F64Container {
    fn remove_all(&mut self) -> f64 {
        if let Some(resource) = self.resource.take() {
            resource
        } else {
            0.0
        }
    }
}


pub struct F64ContainerFactory {
    pub prefix: String,
    pub next_index: u64,
    pub num_digits: usize,
    pub create_quantity_distr: Option<Distribution>,
    pub container_capacity: f64,
}

impl ItemFactory<F64Container> for F64ContainerFactory {
    fn create_item(&mut self) -> F64Container {
        let id = format!("{}{:0width$}", self.prefix, self.next_index, width = self.num_digits);
        self.next_index += 1;
        let resource = if let Some(distr) = &mut self.create_quantity_distr {
            Some(distr.sample())
        } else {
            None
        };
        F64Container {
            id,
            resource,
            capacity: self.container_capacity,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Vector3Container {
    pub id: String,
    pub resource: Option<Vector3>,
    pub capacity: f64,
}

impl ResourceRemoveAll<Vector3> for Vector3Container {
    fn remove_all(&mut self) -> Vector3 {
        if let Some(resource) = self.resource.take() {
            resource
        } else {
            Vector3::default()
        }
    }
}

impl ResourceAdd<Vector3> for Vector3Container {
    fn add(&mut self, arg: Vector3) {
        if let Some(resource) = &mut self.resource {
            resource.add(arg);
        } else {
            self.resource = Some(arg);
        }
    }
}

impl Serialize for Vector3Container {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("Vector3Container", 3)?;
        state.serialize_field("id", &self.id)?;
        state.serialize_field("resource", &self.resource)?;
        state.serialize_field("capacity", &self.capacity)?;
        state.end()
    }
}

#[derive(Debug, Clone, Default)]
pub struct Vector3ContainerFactory {
    pub prefix: String,
    pub next_index: u64,
    pub num_digits: usize,
    pub create_quantity_distr: Option<Distribution>,
    pub create_component_split: [f64; 3],
    pub capacity: f64,
}

impl ItemFactory<Vector3Container> for Vector3ContainerFactory {
    fn create_item(&mut self) -> Vector3Container {
        let id = format!("{}{:0width$}", self.prefix, self.next_index, width = self.num_digits);
        self.next_index += 1;
        let resource = if let Some(distr) = &mut self.create_quantity_distr {
            let quantity = distr.sample();
            Some(Vector3 { values: [
                quantity * self.create_component_split[0],
                quantity * self.create_component_split[1],
                quantity * self.create_component_split[2],
            ]})
        } else {
            None
        };
        Vector3Container {
            id,
            resource,
            capacity: self.capacity,
        }
    }
}

pub struct ContainerLoadingProcess<
    ContainerType: Clone + Send + 'static,
    ResourceType: Clone + Send + 'static,
> {
    pub element_name: String,
    pub element_code: String,
    pub element_type: String,
    pub req_us_containers: Requestor<(), DiscreteStockState>,
    pub req_us_resource: Requestor<(), VectorStockState>,
    pub req_downstream: Requestor<(), DiscreteStockState>,

    pub req_environment: Requestor<(), BasicEnvironmentState>,
    pub env_state: BasicEnvironmentState,

    pub withdraw_us_containers: Requestor<((), EventId), Option<ContainerType>>,
    pub withdraw_us_resource: Requestor<(f64, EventId), ResourceType>,
    pub push_downstream: Output<(ContainerType, EventId)>,

    processes_in_progress: Vec<(Duration, ContainerType)>,
    pub processes_complete: VecDeque<ContainerType>,

    pub max_process_count: usize,
    pub process_time_distr: Option<Distribution>,
    pub process_quantity_distr: Option<Distribution>,

    pub log_emitter: Output<DiscreteProcessLog<ContainerType>>,
    time_to_next_event: Option<Duration>,
    scheduled_event: Option<(MonotonicTime, ActionKey)>,
    next_event_index: u64,
    pub previous_check_time: MonotonicTime,
}

impl<
    ContainerType: Clone + Send + 'static,
    ResourceType: Clone + Send + 'static,
> Default for ContainerLoadingProcess<ContainerType, ResourceType> {
    fn default() -> Self {
        Self {
            element_name: "ContainerLoadingProcess".to_string(),
            element_code: "CLP".to_string(),
            element_type: "ContainerLoadingProcess".to_string(),
            req_us_containers: Requestor::default(),
            req_us_resource: Requestor::default(),
            req_downstream: Requestor::default(),

            req_environment: Requestor::default(),
            env_state: BasicEnvironmentState::Normal,

            withdraw_us_containers: Requestor::default(),
            withdraw_us_resource: Requestor::default(),
            push_downstream: Output::default(),

            max_process_count: 1,
            processes_in_progress: Vec::new(),
            processes_complete: VecDeque::new(),

            process_time_distr: None,
            process_quantity_distr: None,

            log_emitter: Output::default(),
            time_to_next_event: None,
            scheduled_event: None,
            next_event_index: 0,
            previous_check_time: MonotonicTime::EPOCH,
        }
    }
}

impl<
    ContainerType: Clone + Send + ResourceAdd<ResourceType> + 'static,
    ResourceType: Clone + Send + 'static,
> ContainerLoadingProcess<ContainerType, ResourceType> {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn with_name(mut self, name: String) -> Self {
        self.element_name = name;
        self
    }

    pub fn with_code(mut self, code: String) -> Self {
        self.element_code = code;
        self
    }

    pub fn with_type(mut self, element_type: String) -> Self {
        self.element_type = element_type;
        self
    }

    pub fn with_process_time_distr(mut self, distr: Distribution) -> Self {
        self.process_time_distr = Some(distr);
        self
    }

    pub fn with_process_quantity_distr(mut self, distr: Distribution) -> Self {
        self.process_quantity_distr = Some(distr);
        self
    }
}

impl<
    ContainerType: Clone + Send + ResourceAdd<ResourceType> + 'static,
    ResourceType: Clone + Send + 'static,
> Model for ContainerLoadingProcess<ContainerType, ResourceType> {
    fn init(mut self, cx: &mut Context<Self>) -> impl Future<Output = InitializedModel<Self>> + Send {
        async move {
            self.update_state(EventId::from_init(), cx).await;
            self.into()
        }       
    }
}

impl<
    ContainerType: Clone + Send + ResourceAdd<ResourceType> + 'static,
    ResourceType: Clone + Send + 'static,
> Process for ContainerLoadingProcess<ContainerType, ResourceType> {
    type LogDetailsType = DiscreteProcessLogType<ContainerType>;
    
    fn pre_update_state(&mut self, source_event_id: &mut EventId, cx: &mut Context<Self>) -> impl Future<Output = ()> + Send where Self: Model {
        async move {
            if let Some((scheduled_time, _)) = self.scheduled_event.as_ref() {
                if *scheduled_time <= cx.time() {
                    self.scheduled_event = None;
                }
            }
        }
    }
    
    fn update_state_impl(&mut self, source_event_id: &mut EventId, cx: &mut Context<Self>) -> impl Future<Output = ()> + Send where Self: Model {
        async move {
            // First resolve any completed processes

            let time = cx.time();
            let duration_since_prev_check = cx.time().duration_since(self.previous_check_time);
            let new_env_state = match self.req_environment.send(()).await.next() {
                Some(x) => x,
                None => BasicEnvironmentState::Normal // Assume always normal operation if no environment state connected
            };

            match &self.env_state {
                BasicEnvironmentState::Normal => {
                    self.processes_in_progress.retain_mut(|(process_time_left, item)| {
                        *process_time_left = process_time_left.saturating_sub(duration_since_prev_check);
                        if process_time_left.is_zero() {
                            self.processes_complete.push_back(item.clone());
                            false
                        } else {
                            true
                        }
                    });
                },
                BasicEnvironmentState::Stopped => {}
            }

            while let Some(item) = self.processes_complete.pop_front() {
                let ds_state = self.req_downstream.send(()).await.next();
                match &ds_state {
                    Some(DiscreteStockState::Empty { .. } | DiscreteStockState::Normal { .. }) => {
                        *source_event_id = self.log(time, source_event_id.clone(), DiscreteProcessLogType::ProcessFinish { resource: item.clone() }).await;
                        self.push_downstream.send((item.clone(), source_event_id.clone())).await;
                    },
                    Some(DiscreteStockState::Full { .. }) => {
                        *source_event_id = self.log(time, source_event_id.clone(), DiscreteProcessLogType::ProcessNonStart { reason: "Downstream is full" }).await;
                        break;
                    },
                    None => {
                        *source_event_id = self.log(time, source_event_id.clone(), DiscreteProcessLogType::ProcessNonStart { reason: "Downstream is not connected" }).await;
                        break;
                    }
                }
            }

            match (&self.env_state, &new_env_state) {
                (BasicEnvironmentState::Normal, BasicEnvironmentState::Stopped) => {
                    *source_event_id = self.log(time, source_event_id.clone(), DiscreteProcessLogType::ProcessStopped { reason: "Stopped by environment" }).await;
                    self.env_state = BasicEnvironmentState::Stopped;
                },
                (BasicEnvironmentState::Stopped, BasicEnvironmentState::Normal) => {
                    *source_event_id = self.log(time, source_event_id.clone(), DiscreteProcessLogType::ProcessStopped { reason: "Resumed by environment" }).await;
                    self.env_state = BasicEnvironmentState::Normal;
                }
                _ => {}
            }

            // Then check for any processes to start

            match &self.env_state {
                BasicEnvironmentState::Stopped => {
                    self.time_to_next_event = None;
                },
                BasicEnvironmentState::Normal => {
                    loop {
                        let us_state = self.req_us_containers.send(()).await.next();
                        let remaining_proc_count = self.max_process_count - self.processes_in_progress.len() - self.processes_complete.len();
                        match (&us_state, remaining_proc_count) {
                            (Some(DiscreteStockState::Empty { .. } | DiscreteStockState::Normal { .. }), 1..) => {
                                *source_event_id = self.log(time, source_event_id.clone(), DiscreteProcessLogType::WithdrawRequest).await;
                                let container = self.withdraw_us_containers.send(((), source_event_id.clone())).await.next().unwrap();
                                if let Some(mut item) = container {
                                    let quantity = self.process_quantity_distr.as_mut().unwrap_or_else(|| {
                                        panic!("Process quantity distribution not set for process {}", self.element_name);
                                    }).sample();
                                    let resource = self.withdraw_us_resource.send((quantity, source_event_id.clone())).await.next().unwrap();
                                    let process_duration = Duration::from_secs_f64(self.process_time_distr.as_mut().unwrap_or_else(|| {
                                        panic!("Process time distribution not set for process {}", self.element_name);
                                    }).sample());
                                    item.add(resource);
                                    self.processes_in_progress.push((process_duration, item.clone()));
                                    *source_event_id = self.log(time, source_event_id.clone(), DiscreteProcessLogType::ProcessStart { resource: item }).await;

                                } else {
                                    break;
                                }
                            },
                            (_, 0) => {
                                *source_event_id = self.log(time, source_event_id.clone(), DiscreteProcessLogType::ProcessNonStart { reason: "No remaining process slots" }).await;
                                break;
                            },
                            (Some(DiscreteStockState::Full { .. }), _) => {
                                *source_event_id = self.log(time, source_event_id.clone(), DiscreteProcessLogType::ProcessNonStart { reason: "Upstream is full" }).await;
                                break;
                            },
                            (None, _) => {
                                *source_event_id = self.log(time, source_event_id.clone(), DiscreteProcessLogType::ProcessNonStart { reason: "Upstream is not connected" }).await;
                                break;
                            }
                        }
                    }
                    
                    self.time_to_next_event = if self.processes_in_progress.is_empty() {
                        None
                    } else {
                        // Find the minimum time to next event
                        let min_time = self.processes_in_progress.iter().map(|(time, _)| *time).min().unwrap();
                        Some(min_time)
                    };
                }
            }
        }
    }
    
    fn post_update_state(&mut self, source_event_id: &mut EventId, cx: &mut Context<Self>) -> impl Future<Output = ()> + Send where Self: Model {
        async move {
            match self.time_to_next_event {
                None => {},
                Some(time_until_next) => {
                    if time_until_next.is_zero() {
                        panic!("Time until next event is zero!");
                    } else {
                        let next_time = cx.time() + time_until_next;
                        
                        // Schedule event if sooner. If so, cancel previous event.
                        if let Some((scheduled_time, action_key)) = self.scheduled_event.take() {
                            if next_time < scheduled_time {
                                action_key.cancel();
                                let new_event_key =  cx.schedule_keyed_event(next_time, <Self as Process>::update_state, source_event_id.clone()).unwrap();
                                self.scheduled_event = Some((next_time, new_event_key));
                            } else {
                                // Put the event back
                                self.scheduled_event = Some((scheduled_time, action_key));
                            }
                        } else {
                            let new_event_key =  cx.schedule_keyed_event(next_time, <Self as Process>::update_state, source_event_id.clone()).unwrap();
                            self.scheduled_event = Some((next_time, new_event_key));
                        }
                    };
                }
            };
            self.previous_check_time = cx.time();
        }
    }
    
    fn log(&mut self, now: MonotonicTime, source_event_id: EventId, details: Self::LogDetailsType) -> impl Future<Output = EventId> {
        async move {
            let new_event_id = EventId(format!("{}_{:06}", self.element_code, self.next_event_index));
            let log = DiscreteProcessLog {
                time: now.to_chrono_date_time(0).unwrap().to_string(),
                event_id: new_event_id.clone(),
                source_event_id,
                element_name: self.element_name.clone(),
                element_type: self.element_type.clone(),
                event: details,
            };
            self.next_event_index += 1;
            self.log_emitter.send(log).await;

            new_event_id
        }
    }
}





pub struct ContainerUnloadingProcess<
    ContainerType: Clone + Send + 'static,
    ResourceType: Clone + Send + 'static,
> {
    pub element_name: String,
    pub element_code: String,
    pub element_type: String,
    pub req_upstream: Requestor<(), DiscreteStockState>,
    pub req_environment: Requestor<(), BasicEnvironmentState>,
    pub env_state: BasicEnvironmentState,
    pub req_ds_containers: Requestor<(), DiscreteStockState>,
    pub req_ds_resource: Requestor<(), VectorStockState>,

    pub withdraw_upstream: Requestor<((), EventId), Option<ContainerType>>,
    pub push_ds_containers: Output<(ContainerType, EventId)>,
    pub push_ds_resource: Output<(ResourceType, EventId)>,

    processes_in_progress: Vec<(Duration, ContainerType)>,
    pub processes_complete: VecDeque<ContainerType>,

    pub max_process_count: usize,
    pub process_time_distr: Option<Distribution>,

    pub log_emitter: Output<DiscreteProcessLog<ContainerType>>,
    time_to_next_event: Option<Duration>,
    scheduled_event: Option<(MonotonicTime, ActionKey)>,
    next_event_index: u64,
    pub previous_check_time: MonotonicTime,
}

impl<
    ContainerType: Clone + Send + 'static,
    ResourceType: Clone + Send + 'static,
> Default for ContainerUnloadingProcess<ContainerType, ResourceType> {
    fn default() -> Self {
        Self {
            element_name: "ContainerUnloadingProcess".to_string(),
            element_code: "".to_string(),
            element_type: "ContainerUnloadingProcess".to_string(),
            
            req_upstream: Requestor::default(),
            req_environment: Requestor::default(),
            withdraw_upstream: Requestor::default(),

            env_state: BasicEnvironmentState::Normal,

            req_ds_containers: Requestor::default(),
            req_ds_resource: Requestor::default(),
            push_ds_containers: Output::default(),
            push_ds_resource: Output::default(),

            max_process_count: 1,
            processes_in_progress: Vec::new(),
            processes_complete: VecDeque::new(),

            process_time_distr: None,

            log_emitter: Output::default(),
            time_to_next_event: None,
            scheduled_event: None,
            next_event_index: 0,
            previous_check_time: MonotonicTime::EPOCH,
        }
    }
}

impl<
    ContainerType: Clone + Send + ResourceRemoveAll<ResourceType> + 'static,
    ResourceType: Clone + Send + 'static,
> ContainerUnloadingProcess<ContainerType, ResourceType> {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn with_name(mut self, name: String) -> Self {
        self.element_name = name;
        self
    }

    pub fn with_code(mut self, code: String) -> Self {
        self.element_code = code;
        self
    }

    pub fn with_type(mut self, element_type: String) -> Self {
        self.element_type = element_type;
        self
    }

    pub fn with_process_time_distr(mut self, distr: Distribution) -> Self {
        self.process_time_distr = Some(distr);
        self
    }
}

impl<
    ContainerType: Clone + Send + ResourceRemoveAll<ResourceType> + 'static,
    ResourceType: Clone + Send + 'static,
> Model for ContainerUnloadingProcess<ContainerType, ResourceType> {
    fn init(mut self, cx: &mut Context<Self>) -> impl Future<Output = InitializedModel<Self>> + Send {
        async move {
            self.update_state(EventId::from_init(), cx).await;
            self.into()
        }       
    }
}

impl<
    ContainerType: Clone + Send + ResourceRemoveAll<ResourceType> + 'static,
    ResourceType: Clone + Send + 'static,
> Process for ContainerUnloadingProcess<ContainerType, ResourceType> {
    type LogDetailsType = DiscreteProcessLogType<ContainerType>;
    
    fn pre_update_state(&mut self, source_event_id: &mut EventId, cx: &mut Context<Self>) -> impl Future<Output = ()> + Send where Self: Model {
        async move {
            if let Some((scheduled_time, _)) = self.scheduled_event.as_ref() {
                if *scheduled_time <= cx.time() {
                    self.scheduled_event = None;
                }
            }
        }
    }
    
    fn update_state_impl(&mut self, source_event_id: &mut EventId, cx: &mut Context<Self>) -> impl Future<Output = ()> + Send where Self: Model {
        async move {
            // First resolve any completed processes

            let time = cx.time();
            let duration_since_prev_check = cx.time().duration_since(self.previous_check_time);
            let new_env_state = match self.req_environment.send(()).await.next() {
                Some(x) => x,
                None => BasicEnvironmentState::Normal // Assume always normal operation if no environment state connected
            };

            match &self.env_state {
                BasicEnvironmentState::Normal => {
                    self.processes_in_progress.retain_mut(|(process_time_left, item)| {
                        *process_time_left = process_time_left.saturating_sub(duration_since_prev_check);
                        if process_time_left.is_zero() {
                            self.processes_complete.push_back(item.clone());
                            false
                        } else {
                            true
                        }
                    });
                },
                BasicEnvironmentState::Stopped => {}
            }

            while let Some(mut item) = self.processes_complete.pop_front() {
                let ds_containers_state = self.req_ds_containers.send(()).await.next();
                let ds_resource_state = self.req_ds_resource.send(()).await.next();
                match (&ds_containers_state, &ds_resource_state) {
                    (Some(DiscreteStockState::Empty { .. } | DiscreteStockState::Normal { .. }), Some(VectorStockState::Empty { .. } | VectorStockState::Normal { .. }) ) => {
                        *source_event_id = self.log(time, source_event_id.clone(), DiscreteProcessLogType::ProcessFinish { resource: item.clone() }).await;
                        let resource = item.remove_all();
                        self.push_ds_containers.send((item, source_event_id.clone())).await;
                        self.push_ds_resource.send((resource, source_event_id.clone())).await;
                    },
                    (Some(DiscreteStockState::Full { .. }), _) => {
                        *source_event_id = self.log(time, source_event_id.clone(), DiscreteProcessLogType::ProcessNonStart { reason: "Downstream container stock is full" }).await;
                        break;
                    },
                    (_, Some(VectorStockState::Full { .. })) => {
                        *source_event_id = self.log(time, source_event_id.clone(), DiscreteProcessLogType::ProcessNonStart { reason: "Downstream resource stock is full" }).await;
                        break;
                    },
                    (_, _) => {
                        *source_event_id = self.log(time, source_event_id.clone(), DiscreteProcessLogType::ProcessNonStart { reason: "Downstream is not connected" }).await;
                        break;
                    }
                }
            }

            match (&self.env_state, &new_env_state) {
                (BasicEnvironmentState::Normal, BasicEnvironmentState::Stopped) => {
                    *source_event_id = self.log(time, source_event_id.clone(), DiscreteProcessLogType::ProcessStopped { reason: "Stopped by environment" }).await;
                    self.env_state = BasicEnvironmentState::Stopped;
                },
                (BasicEnvironmentState::Stopped, BasicEnvironmentState::Normal) => {
                    *source_event_id = self.log(time, source_event_id.clone(), DiscreteProcessLogType::ProcessStopped { reason: "Resumed by environment" }).await;
                    self.env_state = BasicEnvironmentState::Normal;
                }
                _ => {}
            }

            // Then check for any processes to start

            match &self.env_state {
                BasicEnvironmentState::Stopped => {
                    self.time_to_next_event = None;
                },
                BasicEnvironmentState::Normal => {
                    loop {
                        let us_state = self.req_upstream.send(()).await.next();
                        let remaining_proc_count = self.max_process_count - self.processes_in_progress.len() - self.processes_complete.len();
                        match (&us_state, remaining_proc_count) {
                            (Some(DiscreteStockState::Normal { .. } | DiscreteStockState::Full { .. }), 1..) => {
                                *source_event_id = self.log(time, source_event_id.clone(), DiscreteProcessLogType::WithdrawRequest).await;
                                let container = self.withdraw_upstream.send(((), source_event_id.clone())).await.next().unwrap();
                                if let Some(item) = container {
                                    let process_duration = Duration::from_secs_f64(self.process_time_distr.as_mut().unwrap_or_else(|| {
                                        panic!("Process time distribution not set for process {}", self.element_name);
                                    }).sample());
                                    self.processes_in_progress.push((process_duration, item.clone()));
                                    *source_event_id = self.log(time, source_event_id.clone(), DiscreteProcessLogType::ProcessStart { resource: item }).await;
                                } else {
                                    // Upstream state was not empty, but nothing was returned?
                                    break;
                                }
                            },
                            (_, 0) => {
                                *source_event_id = self.log(time, source_event_id.clone(), DiscreteProcessLogType::ProcessNonStart { reason: "No remaining process slots" }).await;
                                break;
                            },
                            (Some(DiscreteStockState::Empty { .. }), _) => {
                                *source_event_id = self.log(time, source_event_id.clone(), DiscreteProcessLogType::ProcessNonStart { reason: "Upstream is empty" }).await;
                                break;
                            },
                            (None, _) => {
                                *source_event_id = self.log(time, source_event_id.clone(), DiscreteProcessLogType::ProcessNonStart { reason: "Upstream is not connected" }).await;
                                break;
                            }
                        }
                    }
                    
                    self.time_to_next_event = if self.processes_in_progress.is_empty() {
                        None
                    } else {
                        // Find the minimum time to next event
                        let min_time = self.processes_in_progress.iter().map(|(time, _)| *time).min().unwrap();
                        Some(min_time)
                    };
                }
            }
        }
    }
    
    fn post_update_state(&mut self, source_event_id: &mut EventId, cx: &mut Context<Self>) -> impl Future<Output = ()> + Send where Self: Model {
        async move {
            match self.time_to_next_event {
                None => {},
                Some(time_until_next) => {
                    if time_until_next.is_zero() {
                        panic!("Time until next event is zero!");
                    } else {
                        let next_time = cx.time() + time_until_next;
                        
                        // Schedule event if sooner. If so, cancel previous event.
                        if let Some((scheduled_time, action_key)) = self.scheduled_event.take() {
                            if next_time < scheduled_time {
                                action_key.cancel();
                                let new_event_key =  cx.schedule_keyed_event(next_time, <Self as Process>::update_state, source_event_id.clone()).unwrap();
                                self.scheduled_event = Some((next_time, new_event_key));
                            } else {
                                // Put the event back
                                self.scheduled_event = Some((scheduled_time, action_key));
                            }
                        } else {
                            let new_event_key =  cx.schedule_keyed_event(next_time, <Self as Process>::update_state, source_event_id.clone()).unwrap();
                            self.scheduled_event = Some((next_time, new_event_key));
                        }
                    };
                }
            };
            self.previous_check_time = cx.time();
        }
    }
    
    fn log(&mut self, now: MonotonicTime, source_event_id: EventId, details: Self::LogDetailsType) -> impl Future<Output = EventId> {
        async move {
            let new_event_id = EventId(format!("{}_{:06}", self.element_code, self.next_event_index));
            let log = DiscreteProcessLog {
                time: now.to_chrono_date_time(0).unwrap().to_string(),
                event_id: new_event_id.clone(),
                source_event_id,
                element_name: self.element_name.clone(),
                element_type: self.element_type.clone(),
                event: details,
            };
            self.next_event_index += 1;
            self.log_emitter.send(log).await;

            new_event_id
        }
    }
}


