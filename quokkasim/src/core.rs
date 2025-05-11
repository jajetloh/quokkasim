use std::{error::Error, fmt::Debug, fs::File, time::Duration};
use crate::common::{NotificationMetadata};
use csv::WriterBuilder;
use nexosim::{model::{Context, Model}, ports::EventBuffer};
use serde::Serialize;
use tai_time::MonotonicTime;

pub struct SubtractParts<T> {
    pub remaining: T,
    pub subtracted: T,
}

impl VectorArithmetic for f64 {
    fn add(&self, other: &Self) -> Self {
        self + other
    }

    // fn subtract(&self, other: &Self) -> Self {
    //     self - other
    // }

    fn subtract_parts(&self, quantity: f64) -> SubtractParts<Self> {
        SubtractParts { remaining: self - quantity, subtracted: quantity }
    }

    // fn multiply(&self, scalar: f64) -> Self {
    //     self * scalar
    // }

    // fn divide(&self, scalar: f64) -> Self {
    //     self / scalar
    // }

    fn total(&self) -> f64 {
        *self
    }
}

#[derive(Debug, Clone)]
pub struct Vector3 {
    pub values: [f64; 3],
}

impl VectorArithmetic for Vector3 {
    fn add(&self, other: &Self) -> Self {
        Vector3 {
            values: [
                self.values[0] + other.values[0],
                self.values[1] + other.values[1],
                self.values[2] + other.values[2],
            ],
        }
    }

    fn subtract_parts(&self, quantity: f64) -> SubtractParts<Self> {
        let proportion_subtracted = quantity / self.total();
        let proportion_remaining = 1.0 - proportion_subtracted;
        let remaining = Vector3 {
            values: [
                self.values[0] * proportion_remaining,
                self.values[1] * proportion_remaining,
                self.values[2] * proportion_remaining,
            ],
        };
        let subtracted = Vector3 {
            values: [
                self.values[0] * proportion_subtracted,
                self.values[1] * proportion_subtracted,
                self.values[2] * proportion_subtracted,
            ],
        };
        SubtractParts { remaining , subtracted }
    }

    fn total(&self) -> f64 {
        self.values.iter().sum()
    }
}

pub trait VectorArithmetic where Self: Sized {
    fn add(&self, other: &Self) -> Self;
    fn subtract_parts(&self, quantity: f64) -> SubtractParts<Self>;
    fn total(&self) -> f64;
}

pub trait StateEq {
    fn is_same_state(&self, other: &Self) -> bool;
}

/**
 * U: Parameter type when calling add (i.e. passed from Process when withdrawing)
 * V: Parameter type when calling remove (i.e. passed from Process when pushing)
 */
pub trait Stock<T: VectorArithmetic + Clone + Debug, U: Clone + Send, V: Clone + Send> where Self: Model {

    type StockState: StateEq + Clone;
    // type LogDetailsType;

    fn pre_add<'a>(&'a mut self, payload: &'a (U, NotificationMetadata), cx: &'a mut Context<Self>) -> impl Future<Output = ()> + 'a {
        async move {
            self.set_previous_state();
        }
    }

    fn add_impl<'a>(&'a mut self, payload: &'a (U, NotificationMetadata), cx: &'a mut Context<Self>) -> impl Future<Output = ()> + 'a ;

    fn post_add<'a>(&'a mut self, payload: &'a (U, NotificationMetadata), cx: &'a mut Context<Self>) -> impl Future<Output = ()> + 'a {
        async move {
            let current_state = self.get_state();
            let previous_state: &Option<Self::StockState> = self.get_previous_state();
            match previous_state {
                None => {},
                Some(ps) => {
                    let ps = ps.clone();
                    self.log(cx.time(), "StockAdd".into()).await;
                    if !ps.is_same_state(&current_state) {
                        self.log(cx.time(), "StateChange".into()).await;
                        cx.schedule_event(
                            cx.time() + ::std::time::Duration::from_nanos(1), |model: &mut Self, x, cx: &mut Context<Self>| {    
                                model.emit_change(x, cx);
                            }, NotificationMetadata {
                                time: cx.time(),
                                element_from: "X".into(),
                                message: "X".into(),
                            }
                        ).unwrap();
                    } else {
                    }
                }
            }
        }
    }

    fn emit_change(&mut self, payload: NotificationMetadata, cx: &mut Context<Self>);

    fn add<'a>(&'a mut self, payload: (U, NotificationMetadata), cx: &'a mut Context<Self>) -> impl Future<Output = ()> + 'a where U: 'static {
        async move {
            self.pre_add(&payload, cx).await;
            self.add_impl(&payload, cx).await;
            self.post_add(&payload, cx).await;
        }
    }

    fn pre_remove<'a>(&'a mut self, payload: &'a (V, NotificationMetadata), cx: &'a mut Context<Self>) -> impl Future<Output = ()> + 'a {
        async move {
            self.set_previous_state();
        }
    }

    fn remove_impl<'a>(&'a mut self, payload: &'a (V, NotificationMetadata), cx: &'a mut Context<Self>) -> impl Future<Output = T> + 'a;

    fn post_remove<'a>(&'a mut self, payload: &'a (V, NotificationMetadata), cx: &'a mut Context<Self>) -> impl Future<Output = ()> + 'a {
        async move {
            let current_state = self.get_state();
            let previous_state: &Option<Self::StockState> = self.get_previous_state();
            match previous_state {
                None => {},
                Some(ps) => {
                    let ps = ps.clone();
                    self.log(cx.time(), "StockRemove".into()).await;
                    if !ps.is_same_state(&current_state) {
                        self.log(cx.time(), "StateChange".into()).await;
                        cx.schedule_event(
                            cx.time() + ::std::time::Duration::from_nanos(1), |model: &mut Self, x, cx: &mut Context<Self>| {    
                                model.emit_change(x, cx);
                            }, NotificationMetadata {
                                time: cx.time(),
                                element_from: "X".into(),
                                message: "X".into(),
                            }
                        ).unwrap();
                    } else {
                    }
                }
            }
        }
    }

    fn remove<'a>(&'a mut self, payload: (V, NotificationMetadata), cx: &'a mut Context<Self>) -> impl Future<Output = T> + 'a where V: 'static {
        async move {
            self.pre_remove(&payload, cx).await;
            let  result = self.remove_impl(&payload, cx).await;
            self.post_remove(&payload, cx).await;
            result
        }
    }

    fn get_state_async<'a>(&'a mut self) -> impl Future<Output = Self::StockState> + 'a {
        async move {
            self.get_state()
        }
    }
    fn get_state(&mut self) -> Self::StockState;
    fn get_resource(&self) -> &T;
    fn get_previous_state(&mut self) -> &Option<Self::StockState>;
    fn set_previous_state(&mut self);
    fn log(&mut self, time: MonotonicTime, log_type: String) -> impl Future<Output = ()> + Send;
}

pub trait Process<T: VectorArithmetic + Clone + Debug> {

    type LogDetailsType;

    fn set_previous_check_time(&mut self, time: MonotonicTime);

    fn get_time_to_next_event(&mut self) -> &Option<Duration>;

    fn set_time_to_next_event(&mut self, time: Option<Duration>);

    fn pre_update_state<'a>(&'a mut self, notif_meta: &'a NotificationMetadata, cx: &'a mut Context<Self>) -> impl Future<Output = ()> + 'a where Self: Model {
        async move {}
    }

    fn update_state_impl<'a>(&'a mut self, notif_meta: &'a NotificationMetadata, cx: &'a mut Context<Self>) -> impl Future<Output = ()> + 'a where Self: Model {
        async move {}
    }


    fn update_state<'a>(&'a mut self, notif_meta: NotificationMetadata, cx: &'a mut Context<Self>) -> impl Future<Output = ()> + 'a where Self: Model {
        // async move {}
        async move {
            self.pre_update_state(&notif_meta, cx).await;
            self.update_state_impl(&notif_meta, cx).await;
            self.post_update_state(&notif_meta, cx).await;
        }
    }
    // {
    //     self.pre_update_state(item);
    //     self.update_state_impl(item);
    //     self.post_update_state(item);
    // }
    fn post_update_state<'a> (&'a mut self, notif_meta: &'a NotificationMetadata, cx: &'a mut Context<Self>) -> impl Future<Output = ()> + Send + 'a where Self: Model {
        async move {}
    }

    fn log<'a>(&'a mut self, time: MonotonicTime, details: Self::LogDetailsType) -> impl Future<Output = ()> + Send;
    // fn post_update_state<'a> (&'a mut self, notif_meta: &'a NotificationMetadata, cx: &'a mut Context<Self>) -> impl Future<Output = ()> + Send + 'a where Self: Model,  {
    //     // self.set_next_event_time(time);
    //     async move {
            
    //         self.set_previous_check_time(cx.time());
    //         match self.get_time_to_next_event() {
    //             None => {},
    //             Some(time_until_next) => {
    //                 if time_until_next.is_zero() {
    //                     panic!("Time until next event is zero!");
    //                 } else {
    //                     let next_time = cx.time() + *time_until_next;
    //                     cx.schedule_event(MonotonicTime::EPOCH, <Self as Process<T>>::update_state, notif_meta.clone()).unwrap();
    //                     // cx.schedule_event(next_time, <Self as Process<T>>::post_update_state, notif_meta.clone())
    //                 };
    //             }
    //         };
    //     }
    // }
}

pub trait Logger {
    type RecordType: Serialize;
    fn get_name(&self) -> &String;
    fn get_buffer(self) -> EventBuffer<Self::RecordType>;
    fn write_csv(self, dir: String) -> Result<(), Box<dyn Error>>
    where
        Self: Sized,
    {
        let file = File::create(format!("{}/{}.csv", dir, self.get_name()))?;
        let mut writer = WriterBuilder::new().has_headers(true).from_writer(file);
        self.get_buffer().for_each(|log| {
            writer
                .serialize(log)
                .expect("Failed to write log record to CSV file");
        });
        writer.flush()?;
        Ok(())
    }
    fn new(name: String, buffer_size: usize) -> Self;
}

pub trait CustomComponentConnection {
    fn connect_components(a: Self, b: Self) -> Result<(), Box<dyn ::std::error::Error>>;
}

pub trait CustomLoggerConnection<'a> {
    type ComponentType;
    fn connect_logger(a: Self, b: Self::ComponentType) -> Result<(), Box<dyn ::std::error::Error>>;
}

// pub trait Process {
//     fn pre_update_state() {

//     }

//     fn update_state_impl() {}

//     fn post_update_state() {

//     }

//     fn update_state(&mut self) {
//         self.pre_update_state();
//         self.update_state_impl();
//         self.post_update_state();
//     }
// }

// Components and loggers enums must be defined together as logger enum connector method requires the components enum
#[macro_export]
macro_rules! define_model_enums {
    (
        $(#[$components_enum_meta:meta])*
        pub enum $ComponentsName:ident<'a> {
          $(
            $(#[$components_var_meta:meta])*
            $R:ident $( ( $RT:ty, $RT2:ty ) )?
          ),* $(,)?
        }

        $(#[$logger_enum_meta:meta])*
        pub enum $LoggersName:ident<'a> {
            $(
                $(#[$logger_var_meta:meta])*
                $U:ident $( ( $UT:ty ) )?
            ),* $(,)?
        }
    ) => {

        $(#[$components_enum_meta])*
        pub enum $ComponentsName<'a> {
            VectorStockF64(&'a mut $crate::components::vector::VectorStock<f64>, &'a mut ::nexosim::simulation::Address<$crate::components::vector::VectorStock<f64>>),
            VectorProcessF64(&'a mut $crate::components::vector::VectorProcess<f64, f64, f64>, &'a mut ::nexosim::simulation::Address<$crate::components::vector::VectorProcess<f64, f64, f64>>),
            VectorStockVector3(&'a mut $crate::components::vector::VectorStock<Vector3>, &'a mut ::nexosim::simulation::Address<$crate::components::vector::VectorStock<Vector3>>),
            VectorProcessVector3(&'a mut $crate::components::vector::VectorProcess<Vector3, Vector3, f64>, &'a mut ::nexosim::simulation::Address<$crate::components::vector::VectorProcess<Vector3, Vector3, f64>>),
            $(
                $(#[$components_var_meta])*
                $R $( ( $RT, $RT2 ) )?
            ),*
        }
  
        impl<'a> $ComponentsName<'a> {
            pub fn connect_components(
                mut a: $ComponentsName,
                mut b: $ComponentsName
            ) -> Result<(), Box<dyn ::std::error::Error>>{
                use $crate::core::CustomComponentConnection;
                match (a, b) {
                    ($ComponentsName::VectorStockF64(mut a, ad), $ComponentsName::VectorProcessF64(mut b, bd)) => {
                        a.state_emitter.connect($crate::components::vector::VectorProcess::update_state, bd.clone());
                        b.req_upstream.connect($crate::components::vector::VectorStock::get_state_async, ad.clone());
                        b.withdraw_upstream.connect($crate::components::vector::VectorStock::remove, ad.clone());
                        Ok(())
                    },
                    ($ComponentsName::VectorProcessF64(mut a, ad), $ComponentsName::VectorStockF64(mut b, bd)) => {
                        b.state_emitter.connect($crate::components::vector::VectorProcess::update_state, ad.clone());
                        a.req_downstream.connect($crate::components::vector::VectorStock::get_state_async, bd.clone());
                        a.push_downstream.connect($crate::components::vector::VectorStock::add, bd.clone());
                        Ok(())
                    },
                    ($ComponentsName::VectorStockVector3(mut a, ad), $ComponentsName::VectorProcessVector3(mut b, bd)) => {
                        a.state_emitter.connect($crate::components::vector::VectorProcess::update_state, bd.clone());
                        b.req_upstream.connect($crate::components::vector::VectorStock::get_state_async, ad.clone());
                        b.withdraw_upstream.connect($crate::components::vector::VectorStock::remove, ad.clone());
                        Ok(())
                    },
                    ($ComponentsName::VectorProcessVector3(mut a, ad), $ComponentsName::VectorStockVector3(mut b, bd)) => {
                        b.state_emitter.connect($crate::components::vector::VectorProcess::update_state, ad.clone());
                        a.req_downstream.connect($crate::components::vector::VectorStock::get_state_async, bd.clone());
                        a.push_downstream.connect($crate::components::vector::VectorStock::add, bd.clone());
                        Ok(())
                    },
                    (a,b) => {
                        <$ComponentsName as CustomComponentConnection>::connect_components(a, b)
                    }
                }
            }
        }

        $(#[$logger_enum_meta])*
        pub enum $LoggersName<'a> {
            VectorStockLoggerF64(&'a mut $crate::components::vector::VectorStockLogger<f64>),
            VectorStockLoggerVector3(&'a mut $crate::components::vector::VectorStockLogger<Vector3>),
            VectorProcessLoggerF64(&'a mut $crate::components::vector::VectorProcessLogger<f64>),
            VectorProcessLoggerVector3(&'a mut $crate::components::vector::VectorProcessLogger<Vector3>),
            $(
                $(#[$logger_var_meta])*
                $U $( ( $UT ) )?
            ),*
        }

        impl<'a> $LoggersName<'a> {
            pub fn connect_logger(mut a: $LoggersName, mut b: $ComponentsName) -> Result<(), Box<dyn ::std::error::Error>> {
                use $crate::core::CustomLoggerConnection;
                match (a, b) {
                    ($LoggersName::VectorStockLoggerF64(mut a), $ComponentsName::VectorStockF64(mut b, bd)) => {
                        b.log_emitter.connect_sink(&a.buffer);
                        Ok(())
                    },
                    ($LoggersName::VectorProcessLoggerF64(mut a), $ComponentsName::VectorProcessF64(mut b, bd)) => {
                        b.log_emitter.connect_sink(&a.buffer);
                        Ok(())
                    },
                    ($LoggersName::VectorStockLoggerVector3(mut a), $ComponentsName::VectorStockVector3(mut b, bd)) => {
                        b.log_emitter.connect_sink(&a.buffer);
                        Ok(())
                    },
                    ($LoggersName::VectorProcessLoggerVector3(mut a), $ComponentsName::VectorProcessVector3(mut b, bd)) => {
                        b.log_emitter.connect_sink(&a.buffer);
                        Ok(())
                    },
                    (a,b) => <$LoggersName as CustomLoggerConnection>::connect_logger(a, b),
                }
            }
        }
    }
}
