pub use std::fs::File;
pub use std::{future::Future, time::Duration};

pub use csv::Writer;
pub use crate::common::{Distribution, NotificationMetadata, EventLog, DistributionConfig, DistributionFactory, DistributionParametersError};
pub use nexosim::model::{Context, Model};
pub use nexosim::time::MonotonicTime;
pub use nexosim::ports::{EventBuffer, Output, Requestor};
pub use nexosim::simulation::{ActionKey, Mailbox, SimInit};

pub trait ResourceAdd<Rhs> {
    fn add(&mut self, other: Rhs);
}

pub trait ResourceRemove<Rhs, Output> {
    fn sub(&mut self, other: Rhs) -> Output;
}

pub trait ResourceMultiply<Rhs> {
    fn mul(&mut self, other: Rhs) -> Self;
}

pub trait StateEq {
    fn is_same_state(&self, other: &Self) -> bool;
}

pub trait Stock: Model {

    type ResourceType;
    type AddType;
    type RemoveType;
    type RemoveParameterType;
    type StateType: StateEq + std::fmt::Debug;

    fn check_update_state<'a>(
        &mut self,
        notif_meta: NotificationMetadata,
        cx: &'a mut ::nexosim::model::Context<Self>
    ) -> impl Future<Output = ()> + Send;
    fn add(
        &mut self,
        data: (Self::AddType, NotificationMetadata),
        cx: &mut ::nexosim::model::Context<Self>
    ) -> impl Future<Output = ()> + Send;
    fn remove(
        &mut self,
        data: (Self::RemoveParameterType, NotificationMetadata),
        cx: &mut ::nexosim::model::Context<Self>
    ) -> impl Future<Output=Self::RemoveType> + Send;
    fn notify_change(
        &mut self,
        notif_meta: NotificationMetadata,
        cx: &mut ::nexosim::model::Context<Self>
    ) -> impl Future<Output = ()> + Send;
    fn get_state(&mut self) -> impl Future<Output=Self::StateType> + Send;
}

#[macro_export]
macro_rules! define_stock {
    (
        $(#[$attr:meta])*
        name = $struct_name:ident,
        resource_type = $resource_type:ty,
        initial_resource = $initial_resource:expr,
        add_type = $add_type:ty,
        remove_type = $remove_type:ty,
        remove_parameter_type = $remove_parameter_type:ty,
        state_type = $state_type:ty,
        fields = {
            $($field_name:ident : $field_type:ty),*
        },
        get_state_method = $get_state_method:expr,
        check_update_method = $check_update_state:expr,
        log_record_type = $log_record_type:ty,
        log_method = $log_method:expr
    ) => {
        use $crate::core::{Stock};
 
        $(#[$attr])*
        pub struct $struct_name {
            pub element_name: String,
            pub element_type: String,
            pub resource: $resource_type,
            $(pub $field_name: $field_type),*,
            pub log_emitter: ::nexosim::ports::Output<$log_record_type>,
            pub state_emitter: ::nexosim::ports::Output<$crate::core::NotificationMetadata>,
            prev_state: Option<$state_type>,
        }

        impl $crate::core::Model for $struct_name {}

        impl<'a> $struct_name {
            pub fn new() -> Self {
                $struct_name {
                    element_name: stringify!($struct_name).to_string(),
                    element_type: stringify!($struct_name).to_string(),
                    resource: $initial_resource,
                    $($field_name: Default::default()),*,
                    log_emitter: ::nexosim::ports::Output::new(),
                    state_emitter: ::nexosim::ports::Output::new(),
                    prev_state: None,
                }
            }

            pub fn with_name(mut self, name: String) -> Self {
                self.element_name = name;
                return self
            }

            // pub fn with_log_consumer(mut self, logger: &EventLogger<$log_record_type>) -> Self {
            //     self.log_emitter.connect_sink(&logger.buffer);
            //     return self
            // }
            
            paste::item! {
                $(
                    pub fn [<with_ $field_name>](mut self, $field_name: $field_type) -> Self {
                        self.$field_name = $field_name;
                        self
                    }
                )*
            }

            pub fn log(&'a mut self, time: MonotonicTime, log_type: String) -> impl Future<Output = ()> + Send {
                async move {
                    $log_method(self, time, log_type).await;
                }
            }
        }

        use $crate::core::NotificationMetadata;

        impl Stock for $struct_name {
            type ResourceType = $resource_type;
            type AddType = $add_type;
            type RemoveType = $remove_type;
            type RemoveParameterType = $remove_parameter_type;
            type StateType = $state_type;

            fn check_update_state<'a>(
                &mut self,
                notif_meta: $crate::core::NotificationMetadata,
                cx: &'a mut ::nexosim::model::Context<Self>
            ) -> impl Future<Output = ()> + Send {

                async {
                    use $crate::core::StateEq;

                    $check_update_state(self, cx);
                    let current_state = self.get_state().await;
                    match &self.prev_state {
                        None => {
                        },
                        Some(ps) => {
                            if !ps.is_same_state(&current_state) {
                                self.log(cx.time(), "StateChange".to_string()).await;
                                cx.schedule_event(
                                    cx.time() + ::std::time::Duration::from_nanos(1), $struct_name::notify_change, notif_meta
                                ).unwrap();
                            } else {
                            }
                        }
                    }
                    self.prev_state = Some(current_state);
                }
            }

            async fn add(
                &mut self,
                data: (Self::AddType, NotificationMetadata),
                cx: &mut ::nexosim::model::Context<Self>
            ) {
                self.prev_state = Some(self.get_state().await);
                self.resource.add(data.0.clone());
                let new_state = Some(self.get_state().await);
                self.log(cx.time(), "Add".to_string()).await;
                self.check_update_state(data.1, cx).await;
            }

            async fn remove(
                &mut self,
                data: (Self::RemoveParameterType, NotificationMetadata),
                cx: &mut ::nexosim::model::Context<Self>
            ) -> Self::RemoveType {
                self.prev_state = Some(self.get_state().await);
                let result: Self::RemoveType = self.resource.sub(data.0.clone());
                self.log(cx.time(), "Remove".to_string()).await;
                self.check_update_state(data.1, cx).await;
                result
            }

            async fn notify_change(&mut self, notif_meta: NotificationMetadata, cx: &mut ::nexosim::model::Context<Self>) {
                self.state_emitter.send(NotificationMetadata {
                    time: cx.time(),
                    element_from: self.element_name.clone(),
                    message: "State updated".to_string(),
                }).await;
            }
        
            fn get_state(&mut self) -> impl Future<Output=Self::StateType> + Send {
                let get_state_fn = $get_state_method;
                async move {
                    get_state_fn(self)
                }
            }
        }
    };
}

pub trait Source: Model {

    type AddType;
    type AddParameterType;

    fn create(&mut self, args: Self::AddParameterType) -> Self::AddType;

    fn check_update_state<'a>(
        &'a mut self,
        notif_meta: NotificationMetadata,
        cx: &'a mut ::nexosim::model::Context<Self>
    ) -> impl Future<Output = ()> + Send + 'a;
}

#[macro_export]
macro_rules! define_source {
    (
        $(#[$attr:meta])*
        name = $struct_name:ident,
        resource_type = $resource_type:ty,
        stock_state_type = $stock_state_type:ty,
        add_type = $add_type:ty,
        add_parameter_type = $add_parameter_type:ty,
        create_method = $create_method:expr,
        check_update_method = $check_update_method:expr,
        fields = {
            $($field_name:ident : $field_type:ty),*
        },
        log_record_type = $log_record_type:ty,
        log_method = $log_method:expr,
        log_method_parameter_type = $log_method_parameter_type:ty
    ) => {
        use nexosim::ports::Requestor;
        use $crate::core::{Source, Duration};

        $(#[$attr])*
        pub struct $struct_name {
            pub element_name: String,
            pub element_type: String,
            resource: $resource_type,
            
            $(pub $field_name: $field_type,)*

            previous_check_time: Option<MonotonicTime>,

            pub log_emitter: ::nexosim::ports::Output<$log_record_type>,
            next_scheduled_event_time: Option<MonotonicTime>,
            next_scheduled_event_key: Option<$crate::core::ActionKey>,

            time_to_new_remaining: Duration,
            time_to_new_dist: Distribution,

            pub req_downstream: ::nexosim::ports::Requestor<(), $stock_state_type>,
            pub push_downstream: ::nexosim::ports::Output<($add_type, NotificationMetadata)>,
        }

        impl $crate::core::Model for $struct_name {}

        impl $struct_name {

            pub fn new() -> Self {
                $struct_name {
                    element_name: stringify!($struct_name).to_string(),
                    element_type: stringify!($struct_name).to_string(),
                    resource: Default::default(),
                    $($field_name: Default::default(),)*
                    previous_check_time: None,
                    log_emitter: ::nexosim::ports::Output::new(),
                    next_scheduled_event_time: None,
                    next_scheduled_event_key: None,
                    time_to_new_remaining: Duration::ZERO,
                    time_to_new_dist: Distribution::Constant(1.),
                    req_downstream: ::nexosim::ports::Requestor::new(),
                    push_downstream: ::nexosim::ports::Output::new(),
                }
            }

            pub fn with_name(mut self, name: String) -> Self {
                self.element_name = name;
                return self
            }

            pub fn with_time_to_new_dist(mut self, dist: Distribution) -> Self {
                self.time_to_new_dist = dist;
                return self
            }

            // pub fn with_log_consumer(mut self, logger: &EventLogger<$log_record_type>) -> Self {
            //     self.log_emitter.connect_sink(&logger.buffer);
            //     return self
            // }
            
            paste::item! {
                $(
                    pub fn [<with_ $field_name>](mut self, $field_name: $field_type) -> Self {
                        self.$field_name = $field_name;
                        self
                    }
                )*
            }

            pub fn log<'a>(&'a mut self, time: MonotonicTime, details: $log_method_parameter_type) -> impl Future<Output = ()> + Send {
                async move {
                    $log_method(self, time, details).await;
                }
            }
        }

        impl Default for $struct_name {
            fn default() -> Self {
                $struct_name::new()
            }
        }

        impl Source for $struct_name {
            
            type AddType = $add_type;
            type AddParameterType = $add_parameter_type;

            fn create(&mut self, params: Self::AddParameterType) -> Self::AddType {
                $create_method(self, params)
            }

            fn check_update_state<'a>(
                &'a mut self,
                notif_meta: NotificationMetadata,
                cx: &'a mut ::nexosim::model::Context<Self>
            ) -> impl Future<Output = ()> + Send + 'a {
                async move {
                    let current_time = cx.time();
                    let elapsed_time: Duration = match self.previous_check_time {
                        None => Duration::MAX,
                        Some(t) => current_time.duration_since(t),
                    };
                    self.time_to_new_remaining = self.time_to_new_remaining.checked_sub(elapsed_time).unwrap_or(Duration::ZERO);
    
                    if self.time_to_new_remaining.is_zero() {

                        // Swap self for a default instance, so it can be moved into the async block, before passing it back
                        let self_moved = std::mem::take(self);
                        *self = $check_update_method(self_moved, cx.time().clone()).await;
    
                        self.time_to_new_remaining = Duration::from_secs_f64(self.time_to_new_dist.sample());
                        self.next_scheduled_event_time = Some(current_time + self.time_to_new_remaining);
                        let notif_meta = NotificationMetadata {
                            time: current_time,
                            element_from: self.element_name.clone(),
                            message: "Check update state".to_string(),
                        };
                        self.next_scheduled_event_key = Some(cx.schedule_keyed_event(
                            self.next_scheduled_event_time.unwrap(),
                            Self::check_update_state,
                            notif_meta
                        ).unwrap());
                    }
    
                    self.previous_check_time = Some(current_time);
                }
            }
        }
    };
}

pub trait Sink: Model {
    
    type SubtractType;
    type SubtractParameterType;

    fn check_update_state<'a>(
        &'a mut self,
        notif_meta: NotificationMetadata,
        cx: &'a mut ::nexosim::model::Context<Self>
    ) -> impl Future<Output = ()> + Send + 'a;
}

#[macro_export]
macro_rules! define_sink {
    (
        $(#[$attr:meta])*
        name = $struct_name:ident,
        resource_type = $resource_type:ty,
        stock_state_type = $stock_state_type:ty,
        subtract_type = $subtract_type:ty,
        subtract_parameters_type = $subtract_parameters_type:ty,
        check_update_method = $check_update_method:expr,
        fields = {
            $($field_name:ident : $field_type:ty),*
        },
        log_record_type = $log_record_type:ty,
        log_method = $log_method:expr,
        log_method_parameter_type = $log_method_parameter_type:ty
    ) => {
        use $crate::core::Sink;

        $(#[$attr])*
        pub struct $struct_name {
            element_name: String,
            element_type: String,
            resource: $resource_type,

            $(pub $field_name: $field_type,)*

            previous_check_time: Option<MonotonicTime>,

            pub log_emitter: ::nexosim::ports::Output<$log_record_type>,
            next_scheduled_event_time: Option<MonotonicTime>,
            next_scheduled_event_key: Option<$crate::core::ActionKey>,

            time_to_destroy_remaining: Duration,
            time_to_destroy_dist: Distribution,

            pub req_upstream: ::nexosim::ports::Requestor<(), $stock_state_type>,
            pub withdraw_upstream: ::nexosim::ports::Requestor<($subtract_parameters_type, NotificationMetadata), $subtract_type>,
        }

        impl $crate::core::Model for $struct_name {}

        impl $struct_name {
            pub fn new() -> Self {
                $struct_name {
                    element_name: stringify!($struct_name).to_string(),
                    element_type: stringify!($struct_name).to_string(),
                    resource: Default::default(),
                    $($field_name: Default::default(),)*
                    previous_check_time: None,
                    log_emitter: ::nexosim::ports::Output::new(),
                    next_scheduled_event_time: None,
                    next_scheduled_event_key: None,
                    time_to_destroy_remaining: Duration::ZERO,
                    time_to_destroy_dist: Distribution::Constant(1.),

                    req_upstream: ::nexosim::ports::Requestor::new(),
                    withdraw_upstream: ::nexosim::ports::Requestor::new(),
                }
            }

            pub fn with_name(mut self, name: String) -> Self {
                self.element_name = name;
                return self
            }

            pub fn with_time_to_destroy_dist(mut self, dist: Distribution) -> Self {
                self.time_to_destroy_dist = dist;
                return self
            }

            // pub fn with_log_consumer(mut self, logger: &EventLogger<$log_record_type>) -> Self {
            //     self.log_emitter.connect_sink(&logger.buffer);
            //     return self
            // }
            
            paste::item! {
                $(
                    pub fn [<with_ $field_name>](mut self, $field_name: $field_type) -> Self {
                        self.$field_name = $field_name;
                        self
                    }
                )*
            }

            pub fn log<'a>(&'a mut self, time: MonotonicTime, details: $log_method_parameter_type) -> impl Future<Output = ()> + Send {
                async move {
                    $log_method(self, time, details).await;
                }
            }
        }

        impl Default for $struct_name {
            fn default() -> Self {
                $struct_name::new()
            }
        }

        impl Sink for $struct_name {
            type SubtractType = $subtract_type;
            type SubtractParameterType = $subtract_parameters_type;

            fn check_update_state<'a>(
                &'a mut self,
                notif_meta: NotificationMetadata,
                cx: &'a mut ::nexosim::model::Context<Self>
            ) -> impl Future<Output = ()> + Send + 'a {
            
                async move {
                    let current_time = cx.time();
                    let elapsed_time: Duration = match self.previous_check_time {
                        None => Duration::MAX,
                        Some(t) => current_time.duration_since(t),
                    };
                    self.time_to_destroy_remaining = self.time_to_destroy_remaining.checked_sub(elapsed_time).unwrap_or(Duration::ZERO);

                    if self.time_to_destroy_remaining.is_zero() {

                        let self_moved = std::mem::take(self);
                        *self = $check_update_method(self_moved, current_time.clone()).await;

                        self.time_to_destroy_remaining = Duration::from_secs_f64(self.time_to_destroy_dist.sample());
                        self.next_scheduled_event_time = Some(current_time + self.time_to_destroy_remaining);
                        let notif_meta = NotificationMetadata {
                            time: current_time,
                            element_from: self.element_name.clone(),
                            message: "Check update state".to_string(),
                        };
                        self.next_scheduled_event_key = Some(cx.schedule_keyed_event(
                            self.next_scheduled_event_time.unwrap(),
                            Self::check_update_state,
                            notif_meta
                        ).unwrap());
                    }
                    self.previous_check_time = Some(current_time);
                }
            }
        }

    }
}


pub trait Process: Model {

    type ResourceInType;
    type ResourceInParameterType;

    type ResourceOutType;
    
    type StockStateType;

    fn check_update_state<'a>(
        &'a mut self,
        notif_meta: NotificationMetadata,
        cx: &'a mut ::nexosim::model::Context<Self>
    ) -> impl Future<Output = ()> + Send + 'a;

}

#[macro_export]
macro_rules! define_process {
    (
        $(#[$attr:meta])*
        name = $struct_name:ident,

        stock_state_type = $stock_state_type:ty,
        resource_in_type = $resource_in_type:ty,
        resource_in_parameter_type = $resource_in_parameter_type:ty,
        resource_out_type = $resource_out_type:ty,
        resource_out_parameter_type = $resource_out_parameter_type:ty,

        check_update_method = $check_update_method:expr,
        fields = {
            $($field_name:ident : $field_type:ty),*
        },
        log_record_type = $log_record_type:ty,
        log_method = $log_method:expr,
        log_method_parameter_type = $log_method_parameter_type:ty
    ) => {

        use $crate::core::Process;

        $(#[$attr])*
        pub struct $struct_name {
            pub element_name: String,
            pub element_type: String,

            previous_check_time: Option<MonotonicTime>,

            pub log_emitter: ::nexosim::ports::Output<$log_record_type>,
            next_scheduled_event_time: Option<MonotonicTime>,
            next_scheduled_event_key: Option<$crate::core::ActionKey>,
            time_to_next_event_counter: Option<Duration>,
            next_event_index: i64,

            pub req_upstream: ::nexosim::ports::Requestor<(), $stock_state_type>,
            pub withdraw_upstream: ::nexosim::ports::Requestor<($resource_in_parameter_type, NotificationMetadata), $resource_in_type>,

            pub req_downstream: ::nexosim::ports::Requestor<(), $stock_state_type>,
            pub push_downstream: ::nexosim::ports::Output<($resource_out_type, NotificationMetadata)>,

            $(pub $field_name: $field_type,)*
        }

        impl $crate::core::Model for $struct_name {}

        impl $struct_name {
            pub fn new() -> Self {
                $struct_name {
                    element_name: stringify!($struct_name).to_string(),
                    element_type: stringify!($struct_name).to_string(),
                    $($field_name: Default::default(),)*
                    previous_check_time: None,
                    log_emitter: ::nexosim::ports::Output::new(),
                    next_scheduled_event_time: None,
                    next_scheduled_event_key: None,
                    next_event_index: 0,
                    time_to_next_event_counter: Some(Duration::ZERO),
                    req_upstream: ::nexosim::ports::Requestor::new(),
                    withdraw_upstream: ::nexosim::ports::Requestor::new(),
                    req_downstream: ::nexosim::ports::Requestor::new(),
                    push_downstream: ::nexosim::ports::Output::new(),
                }
            }

            pub fn with_name(mut self, name: String) -> Self {
                self.element_name = name;
                return self
            }

            // pub fn with_log_consumer(mut self, logger: &EventLogger<$log_record_type>) -> Self {
            //     self.log_emitter.connect_sink(&logger.buffer);
            //     return self
            // }

            paste::item! {
                $(
                    pub fn [<with_ $field_name>](mut self, $field_name: $field_type) -> Self {
                        self.$field_name = $field_name;
                        self
                    }
                )*
            }

            pub fn log<'a>(&'a mut self, time: MonotonicTime, details: $log_method_parameter_type) -> impl Future<Output = ()> + Send {
                async move {
                    $log_method(self, time, details).await;
                }
            }
                        
            fn get_event_id(&self) -> String {
                format!("{}-{:0>7}", self.element_name, self.next_event_index)
            }
        }

        impl Process for $struct_name {

            type ResourceInType = $resource_in_type;
            type ResourceInParameterType = $resource_in_parameter_type;
            type ResourceOutType = $resource_out_type;
            type StockStateType = $stock_state_type;

            fn check_update_state<'a>(
                &'a mut self,
                notif_meta: NotificationMetadata,
                cx: &'a mut ::nexosim::model::Context<Self>,
            ) -> impl Future<Output = ()> + Send + 'a {
                async move {
                    self.next_event_index += 1;
                    let current_time = cx.time();
                    let self_moved = std::mem::take(self);
                    *self = $check_update_method(self_moved, current_time.clone()).await; 
                    self.previous_check_time = Some(current_time);
                    match self.time_to_next_event_counter {
                        None => {},
                        Some(time_to_next) => {
                            if time_to_next.is_zero() {
                                panic!("self.time_to_next_event_counter was not set by check_update_method!");
                            } else {
                                self.next_scheduled_event_time = Some(current_time + time_to_next);
                                self.next_scheduled_event_key = Some(cx.schedule_keyed_event(
                                    self.next_scheduled_event_time.unwrap(),
                                    Self::check_update_state,
                                    notif_meta
                                ).unwrap());
                            }
                        }
                    }
                }
            }
        }

        impl Default for $struct_name {
            fn default() -> Self {
                $struct_name::new()
            }
        }
    }
}

#[macro_export]
macro_rules! define_combiner_process {
    (
        $(#[$attr:meta])*
        name = $struct_name:ident,

        inflow_stock_state_types = ( $( $inflow_stock_state_types:ty ),+ ),
        resource_in_types = ( $( $resource_in_types:ty ),+ ),
        resource_in_parameter_types = ( $( $resource_in_parameter_types:ty ),+ ),
        outflow_stock_state_type = $outflow_stock_state_type:ty,
        resource_out_type = $resource_out_type:ty,
        resource_out_parameter_type = $resource_out_parameter_type:ty,

        check_update_method = $check_update_method:expr,
        fields = {
            $($field_name:ident : $field_type:ty),*
        },
        log_record_type = $log_record_type:ty,
        log_method = $log_method:expr,
        log_method_parameter_type = $log_method_parameter_type:ty
    ) => {
        $(#[$attr])*
        pub struct $struct_name {
            pub element_name: String,
            pub element_type: String,
            previous_check_time: Option<MonotonicTime>,
        
            pub log_emitter: ::nexosim::ports::Output<$log_record_type>,
            next_scheduled_event_time: Option<MonotonicTime>,
            next_scheduled_event_key: Option<$crate::core::ActionKey>,
            time_to_next_event_counter: Option<Duration>,
            next_event_index: i64,
        
            pub req_upstreams: ( $( ::nexosim::ports::Requestor<(), $inflow_stock_state_types> ),+ ),
            pub withdraw_upstreams: ( $( ::nexosim::ports::Requestor<($resource_in_parameter_types, NotificationMetadata), $resource_in_types> ),+ ),
        
            pub req_downstream: ::nexosim::ports::Requestor<(), $outflow_stock_state_type>,
            pub push_downstream: ::nexosim::ports::Output<($resource_out_type, NotificationMetadata)>,
        
            $(pub $field_name: $field_type,)*
        }
        
        impl $crate::core::Model for $struct_name {}
        
        impl $struct_name {
            pub fn new() -> Self {
                Self {
                    element_name: stringify!($struct_name).to_string(),
                    element_type: stringify!($struct_name).to_string(),
                    previous_check_time: None,
                    log_emitter: ::nexosim::ports::Output::new(),
                    next_scheduled_event_time: None,
                    next_scheduled_event_key: None,
                    time_to_next_event_counter: Some(Duration::from_secs(0)),
                    next_event_index: 0,
                    req_upstreams: Default::default(),
                    withdraw_upstreams: Default::default(),
                    req_downstream: Default::default(),
                    push_downstream: Default::default(),
                    $($field_name: Default::default(),)*
                }
            }
        
            pub fn with_name(mut self, name: String) -> Self {
                self.element_name = name;
                self
            }
        
            // pub fn with_log_consumer(mut self, logger: &EventLogger<$log_record_type>) -> Self {
            //     self.log_emitter.connect_sink(&logger.buffer);
            //     return self
            // }
            
            paste::item! {
                $(
                    pub fn [<with_ $field_name>](mut self, $field_name: $field_type) -> Self {
                        self.$field_name = $field_name;
                        self
                    }
                )*
            }
        
            pub fn check_update_state<'a>(
                &'a mut self,
                notif_meta: NotificationMetadata,
                cx: &'a mut ::nexosim::model::Context<Self>,
            ) -> impl Future<Output = ()> + Send + 'a {
                async move {
                    self.next_event_index += 1;
                    let current_time = cx.time();
                    let self_moved = std::mem::take(self);
                    *self = $check_update_method(self_moved, current_time.clone()).await;
                    self.previous_check_time = Some(current_time);
                    match self.time_to_next_event_counter {
                        None => {},
                        Some(time_to_next) => {
                            if time_to_next.is_zero() {
                                panic!("self.time_to_next_event_counter was not set by check_update_method!");
                            } else {
                                self.next_scheduled_event_time = Some(current_time + time_to_next);
                                self.next_scheduled_event_key = Some(cx.schedule_keyed_event(
                                    self.next_scheduled_event_time.unwrap(),
                                    Self::check_update_state,
                                    notif_meta
                                ).unwrap());
                            }
                        }
                    }
                }
            }

            pub fn log<'a>(&'a mut self, time: MonotonicTime, details: $log_method_parameter_type) -> impl Future<Output = ()> + Send {
                async move {
                    $log_method(self, time, details).await;
                }
            }

            fn get_event_id(&self) -> String {
                format!("{}-{:0>7}", self.element_name, self.next_event_index)
            }
        
        }
        
        impl Default for $struct_name {
            fn default() -> Self {
                Self::new()
            }
        }
    }
}



#[macro_export]
macro_rules! define_splitter_process {
    (
        $(#[$attr:meta])*
        name = $struct_name:ident,

        inflow_stock_state_type = $inflow_stock_state_type:ty,
        resource_in_type = $resource_in_type:ty,
        resource_in_parameter_type = $resource_in_parameter_type:ty,
        outflow_stock_state_types = ( $( $outflow_stock_state_types:ty ),+ ),
        resource_out_types = ( $( $resource_out_types:ty ),+ ), 
        resource_out_parameter_types = ( $( $resource_out_parameter_types:ty ),+ ),

        check_update_method = $check_update_method:expr,
        fields = {
            $($field_name:ident : $field_type:ty),*
        },
        log_record_type = $log_record_type:ty,
        log_method = $log_method:expr,
        log_method_parameter_type = $log_method_parameter_type:ty
    ) => {

        $(#[$attr])*
        pub struct $struct_name {
            pub element_name: String,
            pub element_type: String,
            previous_check_time: Option<MonotonicTime>,
        
            pub log_emitter: ::nexosim::ports::Output<$log_record_type>,
            next_scheduled_event_time: Option<MonotonicTime>,
            next_scheduled_event_key: Option<$crate::core::ActionKey>,
            time_to_next_event_counter: Option<Duration>,
            next_event_index: i64,
        
            pub req_upstream: ::nexosim::ports::Requestor<(),  $inflow_stock_state_type>,
            pub withdraw_upstream: ::nexosim::ports::Requestor<($resource_in_parameter_type, NotificationMetadata), $resource_in_type>,
        
            pub req_downstreams: ( $( ::nexosim::ports::Requestor<(), $outflow_stock_state_types> ),+ ),
            pub push_downstreams: ( $( ::nexosim::ports::Output<($resource_out_parameter_types, NotificationMetadata)> ),+ ),
        
            $(pub $field_name: $field_type,)*
        }
        
        impl $crate::core::Model for $struct_name {}
        
        impl $struct_name {
            pub fn new() -> Self {
                Self {
                    element_name: stringify!($struct_name).to_string(),
                    element_type: stringify!($struct_name).to_string(),
                    previous_check_time: None,
                    log_emitter: ::nexosim::ports::Output::new(),
                    next_scheduled_event_time: None,
                    next_scheduled_event_key: None,
                    time_to_next_event_counter: Some(Duration::from_secs(0)),
                    next_event_index: 0,
                    req_upstream: Default::default(),
                    withdraw_upstream: Default::default(),
                    req_downstreams: Default::default(),
                    push_downstreams: Default::default(),
                    $($field_name: Default::default(),)*
                }
            }
        
            pub fn with_name(mut self, name: String) -> Self {
                self.element_name = name;
                self
            }
        
            // pub fn with_log_consumer(mut self, logger: &EventLogger<$log_record_type>) -> Self {
            //     self.log_emitter.connect_sink(&logger.buffer);
            //     return self
            // }
            
            paste::item! {
                $(
                    pub fn [<with_ $field_name>](mut self, $field_name: $field_type) -> Self {
                        self.$field_name = $field_name;
                        self
                    }
                )*
            }
        
            pub fn check_update_state<'a>(
                &'a mut self,
                notif_meta: NotificationMetadata,
                cx: &'a mut ::nexosim::model::Context<Self>,
            ) -> impl Future<Output = ()> + Send + 'a {
                async move {
                    self.next_event_index += 1;
                    let current_time = cx.time();
                    let elapsed_time: Duration = match self.previous_check_time {
                        None => Duration::MAX,
                        Some(t) => current_time.duration_since(t),
                    };
                    
                    let self_moved = std::mem::take(self);
                    *self = $check_update_method(self_moved, current_time.clone()).await; 
                    self.previous_check_time = Some(current_time);
                    match self.time_to_next_event_counter {
                        None => {},
                        Some(time_to_next) => {
                            if time_to_next.is_zero() {
                                panic!("self.time_to_next_event_counter was not set by check_update_method!");
                            } else {
                                self.next_scheduled_event_time = Some(current_time + time_to_next);
                                self.next_scheduled_event_key = Some(cx.schedule_keyed_event(
                                    self.next_scheduled_event_time.unwrap(),
                                    Self::check_update_state,
                                    notif_meta
                                ).unwrap());
                            }
                        }
                    }
                }
            }

            pub fn log<'a>(&'a mut self, time: MonotonicTime, details: $log_method_parameter_type) -> impl Future<Output = ()> + Send {
                async move {
                    $log_method(self, time, details).await;
                }
            }

            fn get_event_id(&self) -> String {
                format!("{}-{:0>7}", self.element_name, self.next_event_index)
            }
        }
        
        impl Default for $struct_name {
            fn default() -> Self {
                Self::new()
            }
        }
    }
}


#[macro_export]
macro_rules! connect_components {
    () => {
        
    };
}