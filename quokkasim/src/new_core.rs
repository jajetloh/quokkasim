use std::{error::Error, fmt::Debug, fs::File};

use csv::WriterBuilder;
use nexosim::{model::{Context, Model}, ports::EventBuffer};
use serde::Serialize;

use crate::core::NotificationMetadata;

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
    // fn subtract(&self, other: &Self) -> Self;
    // fn multiply(&self, scalar: f64) -> Self;
    // fn divide(&self, scalar: f64) -> Self;
    fn total(&self) -> f64;
}

pub trait Stock<T: VectorArithmetic + Clone + Debug> {
    fn pre_add(&mut self, item: &T) {
        println!("Pre-adding item: {:?}", item.clone());
    }

    fn add_impl(&mut self, item: &T) {
        // For overriding in the concrete implementation
        unimplemented!();
    }

    fn add(&mut self, item: &T) {
        self.pre_add(item);
        self.add_impl(item);
        self.post_add(item);
    }

    fn post_add(&mut self, item: &T) {
        println!("Post-adding item: {:?}", item.clone());
    }
}

// pub trait WithNextEventTime {
//     fn set_next_event_time(&mut self, time: f64);
// }

pub trait Process<T: VectorArithmetic + Clone + Debug> {
    // fn pre_update_state(&mut self, item: &T) {
    //     // println!("Pre-update item: {:?}", item.clone());
    // }

    // fn update_state_impl(&mut self, item: &T) {
    //     // For overriding in the concrete implementation
    //     unimplemented!();
    // }

    fn update_state<'a> (&'a mut self, notif_meta: NotificationMetadata, cx: &'a mut Context<Self>) -> impl Future<Output = ()> + 'a where Self: Model {
        async move {}
        // self.pre_update_state(item);
        // self.update_state_impl(item);
        // self.post_update_state(item);
    }
    // {
    //     self.pre_update_state(item);
    //     self.update_state_impl(item);
    //     self.post_update_state(item);
    // }

    fn post_update_state<'a> (&'a mut self, notif_meta: NotificationMetadata, cx: &'a mut Context<Self>) -> impl Future<Output = ()> + 'a where Self: Model {
        // self.set_next_event_time(time);
        async move {}
    }
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
        pub enum $ComponentsName:ident {
          $(
            $(#[$components_var_meta:meta])*
            $R:ident $( ( $RT:ty ) )?
          ),* $(,)?
        }

        $(#[$logger_enum_meta:meta])*
        pub enum $LoggersName:ident {
            $(
                $(#[$logger_var_meta:meta])*
                $U:ident $( ( $UT:ty ) )?
            ),* $(,)?
        }
    ) => {

        $(#[$components_enum_meta])*
        pub enum $ComponentsName<'a> {
            NewVectorStockF64(&'a mut $crate::components::new_vector::NewVectorStock<f64>, &'a mut ::nexosim::simulation::Address<$crate::components::new_vector::NewVectorStock<f64>>),
            NewVectorProcessF64(&'a mut $crate::components::new_vector::NewVectorProcess<f64>, &'a mut ::nexosim::simulation::Address<$crate::components::new_vector::NewVectorProcess<f64>>),
            NewVectorStockVector3(&'a mut $crate::components::new_vector::NewVectorStock<Vector3>, &'a mut ::nexosim::simulation::Address<$crate::components::new_vector::NewVectorStock<Vector3>>),
            NewVectorProcessVector3(&'a mut $crate::components::new_vector::NewVectorProcess<Vector3>, &'a mut ::nexosim::simulation::Address<$crate::components::new_vector::NewVectorProcess<Vector3>>),
            $(
                $(#[$components_var_meta])*
                $R $( ( $RT ) )?
            ),*
        }
  
        impl<'a> $ComponentsName<'a> {
            pub fn connect_components(
                mut a: $ComponentsName,
                mut b: $ComponentsName
            ) -> Result<(), Box<dyn ::std::error::Error>>{
                use $crate::new_core::CustomComponentConnection;
                match (a, b) {
                    ($ComponentsName::NewVectorStockF64(mut a, ad), $ComponentsName::NewVectorProcessF64(mut b, bd)) => {
                        a.state_emitter.connect($crate::components::new_vector::NewVectorProcess::check_update_state, bd.clone());
                        b.req_upstream.connect($crate::components::new_vector::NewVectorStock::get_state_async, ad.clone());
                        b.withdraw_upstream.connect($crate::components::new_vector::NewVectorStock::remove, ad.clone());
                        Ok(())
                    },
                    ($ComponentsName::NewVectorProcessF64(mut a, ad), $ComponentsName::NewVectorStockF64(mut b, bd)) => {
                        b.state_emitter.connect($crate::components::new_vector::NewVectorProcess::check_update_state, ad.clone());
                        a.req_downstream.connect($crate::components::new_vector::NewVectorStock::get_state_async, bd.clone());
                        a.push_downstream.connect($crate::components::new_vector::NewVectorStock::add, bd.clone());
                        Ok(())
                    },
                    ($ComponentsName::NewVectorStockVector3(mut a, ad), $ComponentsName::NewVectorProcessVector3(mut b, bd)) => {
                        a.state_emitter.connect($crate::components::new_vector::NewVectorProcess::check_update_state, bd.clone());
                        b.req_upstream.connect($crate::components::new_vector::NewVectorStock::get_state_async, ad.clone());
                        b.withdraw_upstream.connect($crate::components::new_vector::NewVectorStock::remove, ad.clone());
                        Ok(())
                    },
                    ($ComponentsName::NewVectorProcessVector3(mut a, ad), $ComponentsName::NewVectorStockVector3(mut b, bd)) => {
                        b.state_emitter.connect($crate::components::new_vector::NewVectorProcess::check_update_state, ad.clone());
                        a.req_downstream.connect($crate::components::new_vector::NewVectorStock::get_state_async, bd.clone());
                        a.push_downstream.connect($crate::components::new_vector::NewVectorStock::add, bd.clone());
                        Ok(())
                    },
                // ($ComponentsName::NewVectorStockF64(a), $ComponentsName::NewVectorStockF64(_)) => Ok(()),
                // (&a, b) => <$ComponentsName as CustomComponentConnection>::connect_components(a,b),
                _ => {
                    Err(Box::new(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!("Cannot connect components"),
                    )))
                }
                }
            }
        }

        $(#[$logger_enum_meta])*
        pub enum $LoggersName {
            NewVectorStockLoggerF64($crate::components::new_vector::NewVectorStockLogger<f64>),
            NewVectorStockLoggerVector3($crate::components::new_vector::NewVectorStockLogger<Vector3>),
            NewVectorProcessLoggerF64($crate::components::new_vector::NewVectorProcessLogger<f64>),
            NewVectorProcessLoggerVector3($crate::components::new_vector::NewVectorProcessLogger<Vector3>),
            $(
                $(#[$var_meta])*
                $U $( ( $UT ) )?
            ),*
        }

        impl $LoggersName {
            pub fn connect_logger(a: $LoggersName, b: $ComponentsName) -> Result<(), Box<dyn ::std::error::Error>> {
                use $crate::new_core::CustomLoggerConnection;
                match (a,b) {
                    ($LoggersName::NewVectorStockLoggerF64(a), $ComponentsName::NewVectorStockF64(_, _)) => Ok(()),

                    (a,b) => <$LoggersName as CustomLoggerConnection>::connect_logger(a,b),
                }
            }
        }
    }
}
