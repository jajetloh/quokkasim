use std::{error::Error, fmt::Debug, fs::File, time::Duration};
use crate::common::{NotificationMetadata};
use csv::WriterBuilder;
use nexosim::{model::{Context, Model}, ports::EventQueue, simulation::ExecutionError};
use serde::{ser::SerializeStruct, Serialize};
use tai_time::MonotonicTime;

pub struct SubtractParts<T, U> {
    pub remaining: T,
    pub subtracted: U,
}

impl VectorArithmetic<f64, f64, f64> for f64 {
    fn add(&mut self, other: Self) {
        *self += other;
    }

    fn subtract(&mut self, quantity: f64) -> f64 {
        *self = *self - quantity;
        quantity
    }

    fn total(&self) -> f64 {
        *self
    }
}

#[derive(Debug, Clone)]
pub struct Vector3 {
    pub values: [f64; 3],
}

impl Serialize for Vector3 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("Vector3", 3)?;
        state.serialize_field("x", &self.values[0])?;
        state.serialize_field("y", &self.values[1])?;
        state.serialize_field("z", &self.values[2])?;
        state.end()
    }
}

impl VectorArithmetic<Vector3, f64, f64> for Vector3 {
    fn add(&mut self, other: Vector3) {
        self.values[0] += other.values[0];
        self.values[1] += other.values[1];
        self.values[2] += other.values[2];
    }

    fn subtract(&mut self, arg: f64) -> Vector3 {
        let proportion_subtracted = arg / self.total();
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
        *self = remaining;
        subtracted
    }

    fn total(&self) -> f64 {
        self.values.iter().sum()
    }
}

impl Into<Vector3> for [f64; 3] {
    fn into(self) -> Vector3 {
        Vector3 { values: self }
    }
}

impl Default for Vector3 {
    fn default() -> Self {
        Vector3 { values: [0.0, 0.0, 0.0] }
    }
}

/**
 * A: Added/removed parameter type
 * B: Subtract parameter type
 * C: Metric type for total
 */
pub trait VectorArithmetic<A, B, C> where Self: Sized {
    fn add(&mut self, arg: A);
    fn subtract(&mut self, arg: B) -> A;
    fn total(&self) -> C;
    fn multiply(&mut self, factor: f64) {}
}

pub trait StateEq {
    fn is_same_state(&self, other: &Self) -> bool;
}

/**
 * T: Resource type
 * U: Received type from process when adding stock
 * V: Received type from process when removing stock
 * W: Returned type to process when removing stock
 * C: Metric type for resource total
 */
pub trait Stock<T: VectorArithmetic<U, V, C> + Clone + Debug + 'static, U: Clone + Send + 'static, V: Clone + Send + 'static, W: Clone + Send + 'static, C: 'static> where Self: Model {

    type StockState: StateEq + Clone + Debug;
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
                        let notif_meta = NotificationMetadata {
                            time: cx.time(),
                            element_from: "X".into(),
                            message: "X".into(),
                        };
                        // State change emission is scheduled 1ns in future to avoid infinite state change loops with processes
                        cx.schedule_event(
                            cx.time() + ::std::time::Duration::from_nanos(1),
                            Self::emit_change,
                            notif_meta,
                        ).unwrap();
                    } else {
                    }
                }
            }
        }
    }

    fn emit_change<'a>(&'a mut self, payload: NotificationMetadata, cx: &'a mut nexosim::model::Context<Self>) -> impl Future<Output = ()> + Send + 'a;

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

    fn remove_impl<'a>(&'a mut self, payload: &'a (V, NotificationMetadata), cx: &'a mut Context<Self>) -> impl Future<Output = W> + 'a;

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
                        let notif_meta = NotificationMetadata {
                            time: cx.time(),
                            element_from: "X".into(),
                            message: "X".into(),
                        };
                        // State change emission is scheduled 1ns in future to avoid infinite state change loops with processes
                        cx.schedule_event(
                            cx.time() + ::std::time::Duration::from_nanos(1),
                            Self::emit_change,
                            notif_meta,
                        ).unwrap();
                    } else {
                    }
                }
            }
        }
    }

    fn remove<'a>(&'a mut self, payload: (V, NotificationMetadata), cx: &'a mut Context<Self>) -> impl Future<Output = W> + 'a where V: 'static {
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

/**
 * T: Resource type
 * U: Added/removed parameter type
 * V: Parameter type to subtract
 * C: Metric type for resource total
 */
pub trait Process<T: VectorArithmetic<U, V, C> + Clone + Debug, U, V, C> {

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
        async move {
            self.pre_update_state(&notif_meta, cx).await;
            self.update_state_impl(&notif_meta, cx).await;
            self.post_update_state(&notif_meta, cx).await;
        }
    }

    fn post_update_state<'a> (&'a mut self, notif_meta: &'a NotificationMetadata, cx: &'a mut Context<Self>) -> impl Future<Output = ()> + Send + 'a where Self: Model {
        async move {}
    }

    fn log<'a>(&'a mut self, time: MonotonicTime, details: Self::LogDetailsType) -> impl Future<Output = ()> + Send;

}

pub trait Logger {
    type RecordType: Serialize + Send + 'static;
    fn get_name(&self) -> &String;
    fn get_buffer(self) -> EventQueue<Self::RecordType>;
    fn write_csv(self, dir: String) -> Result<(), Box<dyn Error>>
    where
        Self: Sized,
    {
        let file = File::create(format!("{}/{}.csv", dir, self.get_name()))?;
        let mut writer = WriterBuilder::new().has_headers(true).from_writer(file);
        self.get_buffer().into_reader().for_each(|log| {
            writer
                .serialize(log)
                .expect("Failed to write log record to CSV file");
        });
        writer.flush()?;
        Ok(())
    }
    fn new(name: String) -> Self;
}

pub trait CustomComponentConnection {
    fn connect_components(a: &mut Self, b: &mut Self, n: Option<usize>) -> Result<(), Box<dyn ::std::error::Error>>;
}

pub trait CustomLoggerConnection {
    type ComponentType;
    fn connect_logger(a: &mut Self, b: &mut Self::ComponentType, n: Option<usize>) -> Result<(), Box<dyn ::std::error::Error>>;
}

pub trait CustomInit {
    fn initialise(&mut self, simu: &mut crate::nexosim::Simulation) -> Result<(), ExecutionError>;
} 

// Components and loggers enums must be defined together as logger enum connector method requires the components enum
#[macro_export]
macro_rules! define_model_enums {
    (
        $(#[$components_enum_meta:meta])*
        pub enum $ComponentModel:ident {
          $(
            $(#[$components_var_meta:meta])*
            $R:ident $( ( $RT:ty, $RT2:ty ) )?
          ),* $(,)?
        }

        $(#[$components_address_enum_meta:meta])*
        pub enum $ComponentModelAddress:ident {
          $(
            $(#[$components_address_var_meta:meta])*
            $Q:ident $( ( $QT:ty ) )?
          ),* $(,)?
        }

        $(#[$logger_enum_meta:meta])*
        pub enum $ComponentLogger:ident {
            $(
                $(#[$logger_var_meta:meta])*
                $U:ident $( ( $UT:ty ) )?
            ),* $(,)?
        }

        // $(#[$model_init_meta:meta])*
        // pub enum $ComponentInit:ident {}

        $(#[$sch_event_config_enum_meta:meta])*
        pub enum $ScheduledEventConfig:ident {
            $(
                $(#[$sch_event_config_meta:meta])*
                $W:ident $( ( $WT:ty ) )?
            ),* $(,)?
        }
    ) => {

        use ::quokkasim::strum_macros::Display;
        use ::quokkasim::core::Logger;

        $(#[$components_enum_meta])*
        #[derive(Display)]
        pub enum $ComponentModel {
            VectorStockF64($crate::components::vector::VectorStock<f64>, $crate::nexosim::Mailbox<$crate::components::vector::VectorStock<f64>>),
            VectorProcessF64($crate::components::vector::VectorProcess<f64, f64, f64>, $crate::nexosim::Mailbox<$crate::components::vector::VectorProcess<f64, f64, f64>>),
            VectorSourceF64($crate::components::vector::VectorSource<f64, f64>, $crate::nexosim::Mailbox<$crate::components::vector::VectorSource<f64, f64>>),
            VectorSinkF64($crate::components::vector::VectorSink<f64>, $crate::nexosim::Mailbox<$crate::components::vector::VectorSink<f64>>),
            VectorCombiner1F64($crate::components::vector::VectorCombiner<f64, f64, f64, 1>, $crate::nexosim::Mailbox<$crate::components::vector::VectorCombiner<f64, f64, f64, 1>>),
            VectorCombiner2F64($crate::components::vector::VectorCombiner<f64, f64, f64, 2>, $crate::nexosim::Mailbox<$crate::components::vector::VectorCombiner<f64, f64, f64, 2>>),
            VectorCombiner3F64($crate::components::vector::VectorCombiner<f64, f64, f64, 3>, $crate::nexosim::Mailbox<$crate::components::vector::VectorCombiner<f64, f64, f64, 3>>),
            VectorCombiner4F64($crate::components::vector::VectorCombiner<f64, f64, f64, 4>, $crate::nexosim::Mailbox<$crate::components::vector::VectorCombiner<f64, f64, f64, 4>>),
            VectorCombiner5F64($crate::components::vector::VectorCombiner<f64, f64, f64, 5>, $crate::nexosim::Mailbox<$crate::components::vector::VectorCombiner<f64, f64, f64, 5>>),
            VectorSplitter1F64($crate::components::vector::VectorSplitter<f64, f64, f64, 1>, $crate::nexosim::Mailbox<$crate::components::vector::VectorSplitter<f64, f64, f64, 1>>),
            VectorSplitter2F64($crate::components::vector::VectorSplitter<f64, f64, f64, 2>, $crate::nexosim::Mailbox<$crate::components::vector::VectorSplitter<f64, f64, f64, 2>>),
            VectorSplitter3F64($crate::components::vector::VectorSplitter<f64, f64, f64, 3>, $crate::nexosim::Mailbox<$crate::components::vector::VectorSplitter<f64, f64, f64, 3>>),
            VectorSplitter4F64($crate::components::vector::VectorSplitter<f64, f64, f64, 4>, $crate::nexosim::Mailbox<$crate::components::vector::VectorSplitter<f64, f64, f64, 4>>),
            VectorSplitter5F64($crate::components::vector::VectorSplitter<f64, f64, f64, 5>, $crate::nexosim::Mailbox<$crate::components::vector::VectorSplitter<f64, f64, f64, 5>>),
            VectorStockVector3($crate::components::vector::VectorStock<Vector3>, $crate::nexosim::Mailbox<$crate::components::vector::VectorStock<Vector3>>),
            VectorProcessVector3($crate::components::vector::VectorProcess<Vector3, Vector3, f64>, $crate::nexosim::Mailbox<$crate::components::vector::VectorProcess<Vector3, Vector3, f64>>),
            VectorSourceVector3($crate::components::vector::VectorSource<Vector3, Vector3>, $crate::nexosim::Mailbox<$crate::components::vector::VectorSource<Vector3, Vector3>>),
            VectorSinkVector3($crate::components::vector::VectorSink<Vector3>, $crate::nexosim::Mailbox<$crate::components::vector::VectorSink<Vector3>>),
            VectorCombiner1Vector3($crate::components::vector::VectorCombiner<Vector3, Vector3, f64, 1>, $crate::nexosim::Mailbox<$crate::components::vector::VectorCombiner<Vector3, Vector3, f64, 1>>),
            VectorCombiner2Vector3($crate::components::vector::VectorCombiner<Vector3, Vector3, f64, 2>, $crate::nexosim::Mailbox<$crate::components::vector::VectorCombiner<Vector3, Vector3, f64, 2>>),
            VectorCombiner3Vector3($crate::components::vector::VectorCombiner<Vector3, Vector3, f64, 3>, $crate::nexosim::Mailbox<$crate::components::vector::VectorCombiner<Vector3, Vector3, f64, 3>>),
            VectorCombiner4Vector3($crate::components::vector::VectorCombiner<Vector3, Vector3, f64, 4>, $crate::nexosim::Mailbox<$crate::components::vector::VectorCombiner<Vector3, Vector3, f64, 4>>),
            VectorCombiner5Vector3($crate::components::vector::VectorCombiner<Vector3, Vector3, f64, 5>, $crate::nexosim::Mailbox<$crate::components::vector::VectorCombiner<Vector3, Vector3, f64, 5>>),
            VectorSplitter1Vector3($crate::components::vector::VectorSplitter<Vector3, Vector3, f64, 1>, $crate::nexosim::Mailbox<$crate::components::vector::VectorSplitter<Vector3, Vector3, f64, 1>>),
            VectorSplitter2Vector3($crate::components::vector::VectorSplitter<Vector3, Vector3, f64, 2>, $crate::nexosim::Mailbox<$crate::components::vector::VectorSplitter<Vector3, Vector3, f64, 2>>),
            VectorSplitter3Vector3($crate::components::vector::VectorSplitter<Vector3, Vector3, f64, 3>, $crate::nexosim::Mailbox<$crate::components::vector::VectorSplitter<Vector3, Vector3, f64, 3>>),
            VectorSplitter4Vector3($crate::components::vector::VectorSplitter<Vector3, Vector3, f64, 4>, $crate::nexosim::Mailbox<$crate::components::vector::VectorSplitter<Vector3, Vector3, f64, 4>>),
            VectorSplitter5Vector3($crate::components::vector::VectorSplitter<Vector3, Vector3, f64, 5>, $crate::nexosim::Mailbox<$crate::components::vector::VectorSplitter<Vector3, Vector3, f64, 5>>),
            DiscreteStockString($crate::components::discrete::DiscreteStock<String>, $crate::nexosim::Mailbox<$crate::components::discrete::DiscreteStock<String>>),
            DiscreteProcessString($crate::components::discrete::DiscreteProcess<Option<String>, (), Option<String>>, $crate::nexosim::Mailbox<$crate::components::discrete::DiscreteProcess<Option<String>, (), Option<String>>>),
            DiscreteParallelProcessString($crate::components::discrete::DiscreteParallelProcess<Option<String>, (), Option<String>>, $crate::nexosim::Mailbox<$crate::components::discrete::DiscreteParallelProcess<Option<String>, (), Option<String>>>),
            DiscreteSourceString($crate::components::discrete::DiscreteSource<Option<String>, StringItemFactory>, $crate::nexosim::Mailbox<$crate::components::discrete::DiscreteSource<Option<String>, StringItemFactory>>),
            DiscreteSinkString($crate::components::discrete::DiscreteSink<Option<String>, ()>, $crate::nexosim::Mailbox<$crate::components::discrete::DiscreteSink<Option<String>, ()>>),
            $(
                $(#[$components_var_meta])*
                $R $( ( $RT, $RT2 ) )?
            ),*
        }
  
        impl $ComponentModel {
            pub fn connect_components(
                mut a: &mut $ComponentModel,
                mut b: &mut $ComponentModel,
                n: Option<usize>,
            ) -> Result<(), Box<dyn ::std::error::Error>>{
                use $crate::core::CustomComponentConnection;
                match (a, b, n) {
                    ($ComponentModel::VectorStockF64(a, ad), $ComponentModel::VectorProcessF64(b, bd), _) => {
                        a.state_emitter.connect($crate::components::vector::VectorProcess::update_state, bd.address());
                        b.req_upstream.connect($crate::components::vector::VectorStock::get_state_async, ad.address());
                        b.withdraw_upstream.connect($crate::components::vector::VectorStock::remove, ad.address());
                        Ok(())
                    },
                    ($ComponentModel::VectorProcessF64(a, ad), $ComponentModel::VectorStockF64(b, bd), _) => {
                        b.state_emitter.connect($crate::components::vector::VectorProcess::update_state, ad.address());
                        a.req_downstream.connect($crate::components::vector::VectorStock::get_state_async, bd.address());
                        a.push_downstream.connect($crate::components::vector::VectorStock::add, bd.address());
                        Ok(())
                    },

                    // VectorSourceF64
                    ($ComponentModel::VectorSourceF64(a, am), $ComponentModel::VectorStockF64(b, bm), _) => {
                        b.state_emitter.connect($crate::components::vector::VectorSource::update_state, am.address());
                        a.req_downstream.connect($crate::components::vector::VectorStock::get_state_async, bm.address());
                        a.push_downstream.connect($crate::components::vector::VectorStock::add, bm.address());
                        Ok(())
                    },
                    // VectorSinkF64
                    ($ComponentModel::VectorStockF64(a, am), $ComponentModel::VectorSinkF64(b, bm), _) => {
                        a.state_emitter.connect($crate::components::vector::VectorSink::update_state, bm.address());
                        b.req_upstream.connect($crate::components::vector::VectorStock::get_state_async, am.address());
                        b.withdraw_upstream.connect($crate::components::vector::VectorStock::remove, am.address());
                        Ok(())
                    },

                    // TODO: Add combiners and splitters for f64

                    ($ComponentModel::VectorStockVector3(a, ad), $ComponentModel::VectorProcessVector3(b, bd), _) => {
                        a.state_emitter.connect($crate::components::vector::VectorProcess::update_state, bd.address());
                        b.req_upstream.connect($crate::components::vector::VectorStock::get_state_async, ad.address());
                        b.withdraw_upstream.connect($crate::components::vector::VectorStock::remove, ad.address());
                        Ok(())
                    },
                    ($ComponentModel::VectorProcessVector3(a, ad), $ComponentModel::VectorStockVector3(b, bd), _) => {
                        b.state_emitter.connect($crate::components::vector::VectorProcess::update_state, ad.address());
                        a.req_downstream.connect($crate::components::vector::VectorStock::get_state_async, bd.address());
                        a.push_downstream.connect($crate::components::vector::VectorStock::add, bd.address());
                        Ok(())
                    },
                    // VectorSourceVector3
                    ($ComponentModel::VectorSourceVector3(a, am), $ComponentModel::VectorStockVector3(b, bm), _) => {
                        b.state_emitter.connect($crate::components::vector::VectorSource::update_state, am.address());
                        a.req_downstream.connect($crate::components::vector::VectorStock::get_state_async, bm.address());
                        a.push_downstream.connect($crate::components::vector::VectorStock::add, bm.address());
                        Ok(())
                    },
                    // VectorSinkVector3
                    ($ComponentModel::VectorStockVector3(a, am), $ComponentModel::VectorSinkVector3(b, bm), _) => {
                        a.state_emitter.connect($crate::components::vector::VectorSink::update_state, bm.address());
                        b.req_upstream.connect($crate::components::vector::VectorStock::get_state_async, am.address());
                        b.withdraw_upstream.connect($crate::components::vector::VectorStock::remove, am.address());
                        Ok(())
                    },

                    // VectorCombiner1Vector3
                    ($ComponentModel::VectorStockVector3(a, am), $ComponentModel::VectorCombiner1Vector3(b, bm), Some(n)) => {
                        a.state_emitter.connect($crate::components::vector::VectorCombiner::update_state, bm.address());
                        b.req_upstreams[n].connect($crate::components::vector::VectorStock::get_state_async, am.address());
                        b.withdraw_upstreams[n].connect($crate::components::vector::VectorStock::remove, am.address());
                        Ok(())
                    },
                    ($ComponentModel::VectorCombiner1Vector3(a, am), $ComponentModel::VectorStockVector3(b, bm), _) => {
                        b.state_emitter.connect($crate::components::vector::VectorCombiner::update_state, am.address());
                        a.req_downstream.connect($crate::components::vector::VectorStock::get_state_async, bm.address());
                        a.push_downstream.connect($crate::components::vector::VectorStock::add, bm.address());
                        Ok(())
                    },
                    // VectorCombiner2Vector3
                    ($ComponentModel::VectorStockVector3(a, am), $ComponentModel::VectorCombiner2Vector3(b, bm), Some(n)) => {
                        a.state_emitter.connect($crate::components::vector::VectorCombiner::update_state, bm.address());
                        b.req_upstreams[n].connect($crate::components::vector::VectorStock::get_state_async, am.address());
                        b.withdraw_upstreams[n].connect($crate::components::vector::VectorStock::remove, am.address());
                        Ok(())
                    },
                    ($ComponentModel::VectorCombiner2Vector3(a, am), $ComponentModel::VectorStockVector3(b, bm), _) => {
                        b.state_emitter.connect($crate::components::vector::VectorCombiner::update_state, am.address());
                        a.req_downstream.connect($crate::components::vector::VectorStock::get_state_async, bm.address());
                        a.push_downstream.connect($crate::components::vector::VectorStock::add, bm.address());
                        Ok(())
                    },
                    // VectorCombiner3Vector3
                    ($ComponentModel::VectorStockVector3(a, am), $ComponentModel::VectorCombiner3Vector3(b, bm), Some(n)) => {
                        a.state_emitter.connect($crate::components::vector::VectorCombiner::update_state, bm.address());
                        b.req_upstreams[n].connect($crate::components::vector::VectorStock::get_state_async, am.address());
                        b.withdraw_upstreams[n].connect($crate::components::vector::VectorStock::remove, am.address());
                        Ok(())
                    },
                    ($ComponentModel::VectorCombiner3Vector3(a, am), $ComponentModel::VectorStockVector3(b, bm), _) => {
                        b.state_emitter.connect($crate::components::vector::VectorCombiner::update_state, am.address());
                        a.req_downstream.connect($crate::components::vector::VectorStock::get_state_async, bm.address());
                        a.push_downstream.connect($crate::components::vector::VectorStock::add, bm.address());
                        Ok(())
                    },
                    // VectorCombiner4Vector3
                    ($ComponentModel::VectorStockVector3(a, am), $ComponentModel::VectorCombiner4Vector3(b, bm), Some(n)) => {
                        a.state_emitter.connect($crate::components::vector::VectorCombiner::update_state, bm.address());
                        b.req_upstreams[n].connect($crate::components::vector::VectorStock::get_state_async, am.address());
                        b.withdraw_upstreams[n].connect($crate::components::vector::VectorStock::remove, am.address());
                        Ok(())
                    },
                    ($ComponentModel::VectorCombiner4Vector3(a, am), $ComponentModel::VectorStockVector3(b, bm), _) => {
                        b.state_emitter.connect($crate::components::vector::VectorCombiner::update_state, am.address());
                        a.req_downstream.connect($crate::components::vector::VectorStock::get_state_async, bm.address());
                        a.push_downstream.connect($crate::components::vector::VectorStock::add, bm.address());
                        Ok(())
                    },
                    // VectorCombiner5Vector3
                    ($ComponentModel::VectorStockVector3(a, am), $ComponentModel::VectorCombiner5Vector3(b, bm), Some(n)) => {
                        a.state_emitter.connect($crate::components::vector::VectorCombiner::update_state, bm.address());
                        b.req_upstreams[n].connect($crate::components::vector::VectorStock::get_state_async, am.address());
                        b.withdraw_upstreams[n].connect($crate::components::vector::VectorStock::remove, am.address());
                        Ok(())
                    },
                    ($ComponentModel::VectorCombiner5Vector3(a, am), $ComponentModel::VectorStockVector3(b, bm), _) => {
                        b.state_emitter.connect($crate::components::vector::VectorCombiner::update_state, am.address());
                        a.req_downstream.connect($crate::components::vector::VectorStock::get_state_async, bm.address());
                        a.push_downstream.connect($crate::components::vector::VectorStock::add, bm.address());
                        Ok(())
                    },


                    // VectorSplitter1Vector3
                    ($ComponentModel::VectorStockVector3(a, amb), $ComponentModel::VectorSplitter1Vector3(b, bmb), _) => {
                        a.state_emitter.connect($crate::components::vector::VectorSplitter::update_state, bmb.address());
                        b.req_upstream.connect($crate::components::vector::VectorStock::get_state_async, amb.address());
                        b.withdraw_upstream.connect($crate::components::vector::VectorStock::remove, amb.address());
                        Ok(())
                    },
                    ($ComponentModel::VectorSplitter1Vector3(a, amb), $ComponentModel::VectorStockVector3(b, bmb), Some(n)) => {
                        b.state_emitter.connect($crate::components::vector::VectorSplitter::update_state, amb.address());
                        a.req_downstreams[n].connect($crate::components::vector::VectorStock::get_state_async, bmb.address());
                        a.push_downstreams[n].connect($crate::components::vector::VectorStock::add, bmb.address());
                        Ok(())
                    },
                    // VectorSplitter2Vector3
                    ($ComponentModel::VectorStockVector3(a, amb), $ComponentModel::VectorSplitter2Vector3(b, bmb), _) => {
                        a.state_emitter.connect($crate::components::vector::VectorSplitter::update_state, bmb.address());
                        b.req_upstream.connect($crate::components::vector::VectorStock::get_state_async, amb.address());
                        b.withdraw_upstream.connect($crate::components::vector::VectorStock::remove, amb.address());
                        Ok(())
                    },
                    ($ComponentModel::VectorSplitter2Vector3(a, amb), $ComponentModel::VectorStockVector3(b, bmb), Some(n)) => {
                        b.state_emitter.connect($crate::components::vector::VectorSplitter::update_state, amb.address());
                        a.req_downstreams[n].connect($crate::components::vector::VectorStock::get_state_async, bmb.address());
                        a.push_downstreams[n].connect($crate::components::vector::VectorStock::add, bmb.address());
                        Ok(())
                    },
                    // VectorSplitter3Vector3
                    ($ComponentModel::VectorStockVector3(a, amb), $ComponentModel::VectorSplitter3Vector3(b, bmb), _) => {
                        a.state_emitter.connect($crate::components::vector::VectorSplitter::update_state, bmb.address());
                        b.req_upstream.connect($crate::components::vector::VectorStock::get_state_async, amb.address());
                        b.withdraw_upstream.connect($crate::components::vector::VectorStock::remove, amb.address());
                        Ok(())
                    },
                    ($ComponentModel::VectorSplitter3Vector3(a, amb), $ComponentModel::VectorStockVector3(b, bmb), Some(n)) => {
                        b.state_emitter.connect($crate::components::vector::VectorSplitter::update_state, amb.address());
                        a.req_downstreams[n].connect($crate::components::vector::VectorStock::get_state_async, bmb.address());
                        a.push_downstreams[n].connect($crate::components::vector::VectorStock::add, bmb.address());
                        Ok(())
                    },
                    // VectorSplitter4Vector3
                    ($ComponentModel::VectorStockVector3(a, amb), $ComponentModel::VectorSplitter4Vector3(b, bmb), _) => {
                        a.state_emitter.connect($crate::components::vector::VectorSplitter::update_state, bmb.address());
                        b.req_upstream.connect($crate::components::vector::VectorStock::get_state_async, amb.address());
                        b.withdraw_upstream.connect($crate::components::vector::VectorStock::remove, amb.address());
                        Ok(())
                    },
                    ($ComponentModel::VectorSplitter4Vector3(a, amb), $ComponentModel::VectorStockVector3(b, bmb), Some(n)) => {
                        b.state_emitter.connect($crate::components::vector::VectorSplitter::update_state, amb.address());
                        a.req_downstreams[n].connect($crate::components::vector::VectorStock::get_state_async, bmb.address());
                        a.push_downstreams[n].connect($crate::components::vector::VectorStock::add, bmb.address());
                        Ok(())
                    },
                    // VectorSplitter5Vector3
                    ($ComponentModel::VectorStockVector3(a, amb), $ComponentModel::VectorSplitter5Vector3(b, bmb), _) => {
                        a.state_emitter.connect($crate::components::vector::VectorSplitter::update_state, bmb.address());
                        b.req_upstream.connect($crate::components::vector::VectorStock::get_state_async, amb.address());
                        b.withdraw_upstream.connect($crate::components::vector::VectorStock::remove, amb.address());
                        Ok(())
                    },
                    ($ComponentModel::VectorSplitter5Vector3(a, amb), $ComponentModel::VectorStockVector3(b, bmb), Some(n)) => {
                        b.state_emitter.connect($crate::components::vector::VectorSplitter::update_state, amb.address());
                        a.req_downstreams[n].connect($crate::components::vector::VectorStock::get_state_async, bmb.address());
                        a.push_downstreams[n].connect($crate::components::vector::VectorStock::add, bmb.address());
                        Ok(())
                    },

                    // Discrete
                    ($ComponentModel::DiscreteStockString(a, ad), $ComponentModel::DiscreteProcessString(b, bd), _) => {
                        a.state_emitter.connect($crate::components::discrete::DiscreteProcess::update_state, bd.address());
                        b.req_upstream.connect($crate::components::discrete::DiscreteStock::get_state_async, ad.address());
                        b.withdraw_upstream.connect($crate::components::discrete::DiscreteStock::remove, ad.address());
                        Ok(())
                    },
                    ($ComponentModel::DiscreteProcessString(a, ad), $ComponentModel::DiscreteStockString(b, bd), _) => {
                        b.state_emitter.connect($crate::components::discrete::DiscreteProcess::update_state, ad.address());
                        a.req_downstream.connect($crate::components::discrete::DiscreteStock::get_state_async, bd.address());
                        a.push_downstream.connect($crate::components::discrete::DiscreteStock::add, bd.address());
                        Ok(())
                    },
                    ($ComponentModel::DiscreteStockString(a, am), $ComponentModel::DiscreteParallelProcessString(b, bm), _) => {
                        a.state_emitter.connect($crate::components::discrete::DiscreteParallelProcess::update_state, bm.address());
                        b.req_upstream.connect($crate::components::discrete::DiscreteStock::get_state_async, am.address());
                        b.withdraw_upstream.connect($crate::components::discrete::DiscreteStock::remove, am.address());
                        Ok(())
                    },
                    ($ComponentModel::DiscreteParallelProcessString(a, am), $ComponentModel::DiscreteStockString(b, bm), _) => {
                        b.state_emitter.connect($crate::components::discrete::DiscreteParallelProcess::update_state, am.address());
                        a.req_downstream.connect($crate::components::discrete::DiscreteStock::get_state_async, bm.address());
                        a.push_downstream.connect($crate::components::discrete::DiscreteStock::add, bm.address());
                        Ok(())
                    },
                    
                    ($ComponentModel::DiscreteSourceString(a, am), $ComponentModel::DiscreteStockString(b, bm), _) => {
                        b.state_emitter.connect($crate::components::discrete::DiscreteSource::update_state, am.address());
                        a.req_downstream.connect($crate::components::discrete::DiscreteStock::get_state_async, bm.address());
                        a.push_downstream.connect($crate::components::discrete::DiscreteStock::add, bm.address());
                        Ok(())
                    },
                    ($ComponentModel::DiscreteStockString(a, am), $ComponentModel::DiscreteSinkString(b, bm), _) => {
                        a.state_emitter.connect($crate::components::discrete::DiscreteSink::update_state, bm.address());
                        b.req_upstream.connect($crate::components::discrete::DiscreteStock::get_state_async, am.address());
                        b.withdraw_upstream.connect($crate::components::discrete::DiscreteStock::remove, am.address());
                        Ok(())
                    },
                    (a,b,n) => {
                        <$ComponentModel as CustomComponentConnection>::connect_components(a, b, n)
                    }
                }
            }

            pub fn register_component(mut sim_init: $crate::nexosim::SimInit, component: Self) -> $crate::nexosim::SimInit {
                
                match component {
                    $ComponentModel::VectorProcessF64(a, mb) => {
                        let name = a.element_name.clone();
                        let addr = mb.address();
                        sim_init = sim_init.add_model(a, mb, name);
                    },
                    $ComponentModel::VectorStockF64(a, mb) => {
                        let name = a.element_name.clone();
                        let addr = mb.address();
                        sim_init = sim_init.add_model(a, mb, name);
                    },
                    $ComponentModel::VectorSourceF64(a, mb) => {
                        let name = a.element_name.clone();
                        let addr = mb.address();
                        sim_init = sim_init.add_model(a, mb, name);
                    },
                    $ComponentModel::VectorSinkF64(a, mb) => {
                        let name = a.element_name.clone();
                        let addr = mb.address();
                        sim_init = sim_init.add_model(a, mb, name);
                    },
                    // TODO: Add combiners and splitters for f64
                    $ComponentModel::VectorStockVector3(a, mb) => {
                        let name = a.element_name.clone();
                        let addr = mb.address();
                        sim_init = sim_init.add_model(a, mb, name);
                    },
                    $ComponentModel::VectorProcessVector3(a, mb) => {
                        let name = a.element_name.clone();
                        let addr = mb.address();
                        sim_init = sim_init.add_model(a, mb, name);
                    },
                    $ComponentModel::VectorSourceVector3(a, mb) => {
                        let name = a.element_name.clone();
                        let addr = mb.address();
                        sim_init = sim_init.add_model(a, mb, name);
                    },
                    $ComponentModel::VectorSinkVector3(a, mb) => {
                        let name = a.element_name.clone();
                        let addr = mb.address();
                        sim_init = sim_init.add_model(a, mb, name);
                    },
                    $ComponentModel::VectorCombiner1Vector3(a, mb) => {
                        let name = a.element_name.clone();
                        let addr = mb.address();
                        sim_init = sim_init.add_model(a, mb, name);
                    },
                    $ComponentModel::VectorCombiner2Vector3(a, mb) => {
                        let name = a.element_name.clone();
                        let addr = mb.address();
                        sim_init = sim_init.add_model(a, mb, name);
                    },
                    $ComponentModel::VectorCombiner3Vector3(a, mb) => {
                        let name = a.element_name.clone();
                        let addr = mb.address();
                        sim_init = sim_init.add_model(a, mb, name);
                    },
                    $ComponentModel::VectorCombiner4Vector3(a, mb) => {
                        let name = a.element_name.clone();
                        let addr = mb.address();
                        sim_init = sim_init.add_model(a, mb, name);
                    },
                    $ComponentModel::VectorCombiner5Vector3(a, mb) => {
                        let name = a.element_name.clone();
                        let addr = mb.address();
                        sim_init = sim_init.add_model(a, mb, name);
                    },
                    $ComponentModel::VectorSplitter1Vector3(a, mb) => {
                        let name = a.element_name.clone();
                        let addr = mb.address();
                        sim_init = sim_init.add_model(a, mb, name);
                    },
                    $ComponentModel::VectorSplitter2Vector3(a, mb) => {
                        let name = a.element_name.clone();
                        let addr = mb.address();
                        sim_init = sim_init.add_model(a, mb, name);
                    },
                    $ComponentModel::VectorSplitter3Vector3(a, mb) => {
                        let name = a.element_name.clone();
                        let addr = mb.address();
                        sim_init = sim_init.add_model(a, mb, name);
                    },
                    $ComponentModel::VectorSplitter4Vector3(a, mb) => {
                        let name = a.element_name.clone();
                        let addr = mb.address();
                        sim_init = sim_init.add_model(a, mb, name);
                    },
                    $ComponentModel::VectorSplitter5Vector3(a, mb) => {
                        let name = a.element_name.clone();
                        let addr = mb.address();
                        sim_init = sim_init.add_model(a, mb, name);
                    },
                    $ComponentModel::DiscreteStockString(a, mb) => {
                        let name = a.element_name.clone();
                        let addr = mb.address();
                        sim_init = sim_init.add_model(a, mb, name);
                    },
                    $ComponentModel::DiscreteProcessString(a, mb) => {
                        let name = a.element_name.clone();
                        let addr = mb.address();
                        sim_init = sim_init.add_model(a, mb, name);
                    },
                    $ComponentModel::DiscreteParallelProcessString(a, mb) => {
                        let name = a.element_name.clone();
                        let addr = mb.address();
                        sim_init = sim_init.add_model(a, mb, name);
                    },
                    $ComponentModel::DiscreteSourceString(a, mb) => {
                        let name = a.element_name.clone();
                        let addr = mb.address();
                        sim_init = sim_init.add_model(a, mb, name);
                    },
                    $ComponentModel::DiscreteSinkString(a, mb) => {
                        let name = a.element_name.clone();
                        let addr = mb.address();
                        sim_init = sim_init.add_model(a, mb, name);
                    },
                    $(
                        $ComponentModel::$R(a, mb) => {
                            let name = a.element_name.clone();
                            let addr = mb.address();
                            sim_init = sim_init.add_model(a, mb, name);
                        }
                    ),*
                    _ => {
                        panic!("Component type {} not implemented for registration", component);
                    }
                };
                sim_init
            }

            pub fn get_address(&self) -> $ComponentModelAddress {
                match self {
                    $ComponentModel::VectorStockF64(_, mb) => $ComponentModelAddress::VectorStockF64(mb.address()),
                    $ComponentModel::VectorProcessF64(_, mb) => $ComponentModelAddress::VectorProcessF64(mb.address()),
                    $ComponentModel::VectorSourceF64(_, mb) => $ComponentModelAddress::VectorSourceF64(mb.address()),
                    $ComponentModel::VectorSinkF64(_, mb) => $ComponentModelAddress::VectorSinkF64(mb.address()),
                    $ComponentModel::VectorCombiner1F64(_, mb) => $ComponentModelAddress::VectorCombiner1F64(mb.address()),
                    $ComponentModel::VectorCombiner2F64(_, mb) => $ComponentModelAddress::VectorCombiner2F64(mb.address()),
                    $ComponentModel::VectorCombiner3F64(_, mb) => $ComponentModelAddress::VectorCombiner3F64(mb.address()),
                    $ComponentModel::VectorCombiner4F64(_, mb) => $ComponentModelAddress::VectorCombiner4F64(mb.address()),
                    $ComponentModel::VectorCombiner5F64(_, mb) => $ComponentModelAddress::VectorCombiner5F64(mb.address()),
                    $ComponentModel::VectorSplitter1F64(_, mb) => $ComponentModelAddress::VectorSplitter1F64(mb.address()),
                    $ComponentModel::VectorSplitter2F64(_, mb) => $ComponentModelAddress::VectorSplitter2F64(mb.address()),
                    $ComponentModel::VectorSplitter3F64(_, mb) => $ComponentModelAddress::VectorSplitter3F64(mb.address()),
                    $ComponentModel::VectorSplitter4F64(_, mb) => $ComponentModelAddress::VectorSplitter4F64(mb.address()),
                    $ComponentModel::VectorSplitter5F64(_, mb) => $ComponentModelAddress::VectorSplitter5F64(mb.address()),
                    $ComponentModel::VectorStockVector3(_, mb) => $ComponentModelAddress::VectorStockVector3(mb.address()),
                    $ComponentModel::VectorProcessVector3(_, mb) => $ComponentModelAddress::VectorProcessVector3(mb.address()),
                    $ComponentModel::VectorSourceVector3(_, mb) => $ComponentModelAddress::VectorSourceVector3(mb.address()),
                    $ComponentModel::VectorSinkVector3(_, mb) => $ComponentModelAddress::VectorSinkVector3(mb.address()),
                    $ComponentModel::VectorCombiner1Vector3(_, mb) => $ComponentModelAddress::VectorCombiner1Vector3(mb.address()),
                    $ComponentModel::VectorCombiner2Vector3(_, mb) => $ComponentModelAddress::VectorCombiner2Vector3(mb.address()),
                    $ComponentModel::VectorCombiner3Vector3(_, mb) => $ComponentModelAddress::VectorCombiner3Vector3(mb.address()),
                    $ComponentModel::VectorCombiner4Vector3(_, mb) => $ComponentModelAddress::VectorCombiner4Vector3(mb.address()),
                    $ComponentModel::VectorCombiner5Vector3(_, mb) => $ComponentModelAddress::VectorCombiner5Vector3(mb.address()),
                    $ComponentModel::VectorSplitter1Vector3(_, mb) => $ComponentModelAddress::VectorSplitter1Vector3(mb.address()),
                    $ComponentModel::VectorSplitter2Vector3(_, mb) => $ComponentModelAddress::VectorSplitter2Vector3(mb.address()),
                    $ComponentModel::VectorSplitter3Vector3(_, mb) => $ComponentModelAddress::VectorSplitter3Vector3(mb.address()),
                    $ComponentModel::VectorSplitter4Vector3(_, mb) => $ComponentModelAddress::VectorSplitter4Vector3(mb.address()),
                    $ComponentModel::VectorSplitter5Vector3(_, mb) => $ComponentModelAddress::VectorSplitter5Vector3(mb.address()),
                    $ComponentModel::DiscreteStockString(_, mb) => $ComponentModelAddress::DiscreteStockString(mb.address()),
                    $ComponentModel::DiscreteProcessString(_, mb) => $ComponentModelAddress::DiscreteProcessString(mb.address()),
                    $ComponentModel::DiscreteParallelProcessString(_, mb) => $ComponentModelAddress::DiscreteParallelProcessString(mb.address()),
                    $ComponentModel::DiscreteSourceString(_, mb) => $ComponentModelAddress::DiscreteSourceString(mb.address()),
                    $ComponentModel::DiscreteSinkString(_, mb) => $ComponentModelAddress::DiscreteSinkString(mb.address()),
                    $(
                        $(#[$components_var_meta])*
                        $ComponentModel::$R(_, mb) => {
                            $ComponentModelAddress::$R(mb.address())
                        }
                    ),*
                    x => {
                        panic!("Component type {} not implemented for address retrieval", x);
                    }
                }
            }
        }

        #[derive(Display)]
        pub enum $ComponentModelAddress {
            VectorStockF64($crate::nexosim::Address<$crate::components::vector::VectorStock<f64>>),
            VectorProcessF64($crate::nexosim::Address<$crate::components::vector::VectorProcess<f64, f64, f64>>),
            VectorSourceF64($crate::nexosim::Address<$crate::components::vector::VectorSource<f64, f64>>),
            VectorSinkF64($crate::nexosim::Address<$crate::components::vector::VectorSink<f64>>),
            VectorCombiner1F64($crate::nexosim::Address<$crate::components::vector::VectorCombiner<f64, f64, f64, 1>>),
            VectorCombiner2F64($crate::nexosim::Address<$crate::components::vector::VectorCombiner<f64, f64, f64, 2>>),
            VectorCombiner3F64($crate::nexosim::Address<$crate::components::vector::VectorCombiner<f64, f64, f64, 3>>),
            VectorCombiner4F64($crate::nexosim::Address<$crate::components::vector::VectorCombiner<f64, f64, f64, 4>>),
            VectorCombiner5F64($crate::nexosim::Address<$crate::components::vector::VectorCombiner<f64, f64, f64, 5>>),
            VectorSplitter1F64($crate::nexosim::Address<$crate::components::vector::VectorSplitter<f64, f64, f64, 1>>),
            VectorSplitter2F64($crate::nexosim::Address<$crate::components::vector::VectorSplitter<f64, f64, f64, 2>>),
            VectorSplitter3F64($crate::nexosim::Address<$crate::components::vector::VectorSplitter<f64, f64, f64, 3>>),
            VectorSplitter4F64($crate::nexosim::Address<$crate::components::vector::VectorSplitter<f64, f64, f64, 4>>),
            VectorSplitter5F64($crate::nexosim::Address<$crate::components::vector::VectorSplitter<f64, f64, f64, 5>>),
            VectorStockVector3($crate::nexosim::Address<$crate::components::vector::VectorStock<Vector3>>),
            VectorProcessVector3($crate::nexosim::Address<$crate::components::vector::VectorProcess<Vector3, Vector3, f64>>),
            VectorSourceVector3($crate::nexosim::Address<$crate::components::vector::VectorSource<Vector3, Vector3>>),
            VectorSinkVector3($crate::nexosim::Address<$crate::components::vector::VectorSink<Vector3>>),
            VectorCombiner1Vector3($crate::nexosim::Address<$crate::components::vector::VectorCombiner<Vector3, Vector3, f64, 1>>),
            VectorCombiner2Vector3($crate::nexosim::Address<$crate::components::vector::VectorCombiner<Vector3, Vector3, f64, 2>>),
            VectorCombiner3Vector3($crate::nexosim::Address<$crate::components::vector::VectorCombiner<Vector3, Vector3, f64, 3>>),
            VectorCombiner4Vector3($crate::nexosim::Address<$crate::components::vector::VectorCombiner<Vector3, Vector3, f64, 4>>),
            VectorCombiner5Vector3($crate::nexosim::Address<$crate::components::vector::VectorCombiner<Vector3, Vector3, f64, 5>>),
            VectorSplitter1Vector3($crate::nexosim::Address<$crate::components::vector::VectorSplitter<Vector3, Vector3, f64, 1>>),
            VectorSplitter2Vector3($crate::nexosim::Address<$crate::components::vector::VectorSplitter<Vector3, Vector3, f64, 2>>),
            VectorSplitter3Vector3($crate::nexosim::Address<$crate::components::vector::VectorSplitter<Vector3, Vector3, f64, 3>>),
            VectorSplitter4Vector3($crate::nexosim::Address<$crate::components::vector::VectorSplitter<Vector3, Vector3, f64, 4>>),
            VectorSplitter5Vector3($crate::nexosim::Address<$crate::components::vector::VectorSplitter<Vector3, Vector3, f64, 5>>),
            DiscreteStockString($crate::nexosim::Address<$crate::components::discrete::DiscreteStock<String>>),
            DiscreteProcessString($crate::nexosim::Address<$crate::components::discrete::DiscreteProcess<Option<String>, (), Option<String>>>),
            DiscreteParallelProcessString($crate::nexosim::Address<$crate::components::discrete::DiscreteParallelProcess<Option<String>, (), Option<String>>>),
            DiscreteSourceString($crate::nexosim::Address<$crate::components::discrete::DiscreteSource<Option<String>, StringItemFactory>>),
            DiscreteSinkString($crate::nexosim::Address<$crate::components::discrete::DiscreteSink<Option<String>, ()>>),
            $(
                $Q $( ($QT) )?
            ),*
        }

        $(#[$logger_enum_meta])*
        #[derive(Display)]
        pub enum $ComponentLogger {
            VectorStockLoggerF64($crate::components::vector::VectorStockLogger<f64>),
            VectorProcessLoggerF64($crate::components::vector::VectorProcessLogger<f64>),

            VectorStockLoggerVector3($crate::components::vector::VectorStockLogger<Vector3>),
            VectorProcessLoggerVector3($crate::components::vector::VectorProcessLogger<Vector3>),

            DiscreteStockLoggerString($crate::components::discrete::DiscreteStockLogger<String>),
            DiscreteProcessLoggerString($crate::components::discrete::DiscreteProcessLogger<Option<String>>),
            $(
                $(#[$logger_var_meta])*
                $U $( ( $UT ) )?
            ),*
        }

        impl $ComponentLogger {
            pub fn connect_logger(a: &mut $ComponentLogger, b: &mut $ComponentModel, n: Option<usize>) -> Result<(), Box<dyn ::std::error::Error>> {
                use $crate::core::CustomLoggerConnection;
                match (a, b, n) {
                    ($ComponentLogger::VectorStockLoggerF64(a), $ComponentModel::VectorStockF64(b, bd), _) => {
                        b.log_emitter.connect_sink(&a.buffer);
                        Ok(())
                    },
                    ($ComponentLogger::VectorProcessLoggerF64(a), $ComponentModel::VectorProcessF64(b, bd), _) => {
                        b.log_emitter.connect_sink(&a.buffer);
                        Ok(())
                    },
                    ($ComponentLogger::VectorProcessLoggerF64(a), $ComponentModel::VectorSourceF64(b, bd), _) => {
                        b.log_emitter.connect_sink(&a.buffer);
                        Ok(())
                    },
                    ($ComponentLogger::VectorProcessLoggerF64(a), $ComponentModel::VectorSinkF64(b, bd), _) => {
                        b.log_emitter.connect_sink(&a.buffer);
                        Ok(())
                    },
                    // TODO: Add f64 combiners splitters
                    ($ComponentLogger::VectorStockLoggerVector3(a), $ComponentModel::VectorStockVector3(b, bd), _) => {
                        b.log_emitter.connect_sink(&a.buffer);
                        Ok(())
                    },
                    ($ComponentLogger::VectorProcessLoggerVector3(a), $ComponentModel::VectorProcessVector3(b, bd), _) => {
                        b.log_emitter.connect_sink(&a.buffer);
                        Ok(())
                    },
                    ($ComponentLogger::VectorProcessLoggerVector3(a), $ComponentModel::VectorSourceVector3(b, bd), _) => {
                        b.log_emitter.connect_sink(&a.buffer);
                        Ok(())
                    },
                    ($ComponentLogger::VectorProcessLoggerVector3(a), $ComponentModel::VectorSinkVector3(b, bd), _) => {
                        b.log_emitter.connect_sink(&a.buffer);
                        Ok(())
                    },
                    ($ComponentLogger::VectorProcessLoggerVector3(a), $ComponentModel::VectorCombiner1Vector3(b, bmb), _) => {
                        b.log_emitter.connect_sink(&a.buffer);
                        Ok(())
                    },
                    ($ComponentLogger::VectorProcessLoggerVector3(a), $ComponentModel::VectorCombiner2Vector3(b, bmb), _) => {
                        b.log_emitter.connect_sink(&a.buffer);
                        Ok(())
                    },
                    ($ComponentLogger::VectorProcessLoggerVector3(a), $ComponentModel::VectorCombiner3Vector3(b, bmb), _) => {
                        b.log_emitter.connect_sink(&a.buffer);
                        Ok(())
                    },
                    ($ComponentLogger::VectorProcessLoggerVector3(a), $ComponentModel::VectorCombiner4Vector3(b, bmb), _) => {
                        b.log_emitter.connect_sink(&a.buffer);
                        Ok(())
                    },
                    ($ComponentLogger::VectorProcessLoggerVector3(a), $ComponentModel::VectorCombiner5Vector3(b, bmb), _) => {
                        b.log_emitter.connect_sink(&a.buffer);
                        Ok(())
                    },
                    ($ComponentLogger::VectorProcessLoggerVector3(a), $ComponentModel::VectorSplitter1Vector3(b, bmb), _) => {
                        b.log_emitter.connect_sink(&a.buffer);
                        Ok(())
                    },
                    ($ComponentLogger::VectorProcessLoggerVector3(a), $ComponentModel::VectorSplitter2Vector3(b, bmb), _) => {
                        b.log_emitter.connect_sink(&a.buffer);
                        Ok(())
                    },
                    ($ComponentLogger::VectorProcessLoggerVector3(a), $ComponentModel::VectorSplitter3Vector3(b, bmb), _) => {
                        b.log_emitter.connect_sink(&a.buffer);
                        Ok(())
                    },
                    ($ComponentLogger::VectorProcessLoggerVector3(a), $ComponentModel::VectorSplitter4Vector3(b, bmb), _) => {
                        b.log_emitter.connect_sink(&a.buffer);
                        Ok(())
                    },
                    ($ComponentLogger::VectorProcessLoggerVector3(a), $ComponentModel::VectorSplitter5Vector3(b, bmb), _) => {
                        b.log_emitter.connect_sink(&a.buffer);
                        Ok(())
                    },

                    ($ComponentLogger::DiscreteStockLoggerString(a), $ComponentModel::DiscreteStockString(b, bd), _) => {
                        b.log_emitter.connect_sink(&a.buffer);
                        Ok(())
                    },
                    ($ComponentLogger::DiscreteProcessLoggerString(a), $ComponentModel::DiscreteProcessString(b, bd), _) => {
                        b.log_emitter.connect_sink(&a.buffer);
                        Ok(())
                    },
                    ($ComponentLogger::DiscreteProcessLoggerString(a), $ComponentModel::DiscreteParallelProcessString(b, bd), _) => {
                        b.log_emitter.connect_sink(&a.buffer);
                        Ok(())
                    },
                    ($ComponentLogger::DiscreteProcessLoggerString(a), $ComponentModel::DiscreteSourceString(b, bd), _) => {
                        b.log_emitter.connect_sink(&a.buffer);
                        Ok(())
                    },
                    ($ComponentLogger::DiscreteProcessLoggerString(a), $ComponentModel::DiscreteSinkString(b, bd), _) => {
                        b.log_emitter.connect_sink(&a.buffer);
                        Ok(())
                    },
                    (a,b,n) => <$ComponentLogger as CustomLoggerConnection>::connect_logger(a, b, n)
                }
            }

            pub fn write_csv(mut self, dir: &str) -> Result<(), Box<dyn Error>> {
                match self {
                    $ComponentLogger::VectorStockLoggerF64(a) => { a.write_csv(dir.to_string()) },
                    $ComponentLogger::VectorProcessLoggerF64(a) => { a.write_csv(dir.to_string()) },

                    $ComponentLogger::VectorStockLoggerVector3(a) => { a.write_csv(dir.to_string()) },
                    $ComponentLogger::VectorProcessLoggerVector3(a) => { a.write_csv(dir.to_string()) },

                    $ComponentLogger::DiscreteStockLoggerString(a) => { a.write_csv(dir.to_string()) },
                    $ComponentLogger::DiscreteProcessLoggerString(a) => { a.write_csv(dir.to_string()) },

                    $(
                        $ComponentLogger::$U (a) => {
                            a.write_csv(dir.to_string())
                        }
                    ),*
                    _ => {
                        Err(format!("Logger type {} not implemented for writing CSV", self).into())
                    }
                }
            } 
        }

        impl $ComponentModelAddress {
            fn initialise(&mut self, simu: &mut $crate::nexosim::Simulation) -> Result<(), $crate::nexosim::ExecutionError> {
                let notif_meta = NotificationMetadata {
                    time: simu.time(),
                    element_from: "Init".into(),
                    message: "Init".into(),
                }; 
                match self {
                    $ComponentModelAddress::VectorStockF64(a) => { Ok(()) },
                    $ComponentModelAddress::VectorProcessF64(a) => { simu.process_event($crate::components::vector::VectorProcess::<f64, f64, f64>::update_state, notif_meta, a.clone()) },
                    $ComponentModelAddress::VectorSourceF64(a) => { simu.process_event($crate::components::vector::VectorSource::<f64, f64>::update_state, notif_meta, a.clone()) },
                    $ComponentModelAddress::VectorSinkF64(a) => { simu.process_event($crate::components::vector::VectorSink::<f64>::update_state, notif_meta, a.clone()) },
                    $ComponentModelAddress::VectorCombiner1F64(a) => { simu.process_event($crate::components::vector::VectorCombiner::<f64, f64, f64, 1>::update_state, notif_meta, a.clone()) },
                    $ComponentModelAddress::VectorCombiner2F64(a) => { simu.process_event($crate::components::vector::VectorCombiner::<f64, f64, f64, 2>::update_state, notif_meta, a.clone()) },
                    $ComponentModelAddress::VectorCombiner3F64(a) => { simu.process_event($crate::components::vector::VectorCombiner::<f64, f64, f64, 3>::update_state, notif_meta, a.clone()) },
                    $ComponentModelAddress::VectorCombiner4F64(a) => { simu.process_event($crate::components::vector::VectorCombiner::<f64, f64, f64, 4>::update_state, notif_meta, a.clone()) },
                    $ComponentModelAddress::VectorCombiner5F64(a) => { simu.process_event($crate::components::vector::VectorCombiner::<f64, f64, f64, 5>::update_state, notif_meta, a.clone()) },
                    $ComponentModelAddress::VectorSplitter1F64(a) => { simu.process_event($crate::components::vector::VectorSplitter::<f64, f64, f64, 1>::update_state, notif_meta, a.clone()) },
                    $ComponentModelAddress::VectorSplitter2F64(a) => { simu.process_event($crate::components::vector::VectorSplitter::<f64, f64, f64, 2>::update_state, notif_meta, a.clone()) },
                    $ComponentModelAddress::VectorSplitter3F64(a) => { simu.process_event($crate::components::vector::VectorSplitter::<f64, f64, f64, 3>::update_state, notif_meta, a.clone()) },
                    $ComponentModelAddress::VectorSplitter4F64(a) => { simu.process_event($crate::components::vector::VectorSplitter::<f64, f64, f64, 4>::update_state, notif_meta, a.clone()) },
                    $ComponentModelAddress::VectorSplitter5F64(a) => { simu.process_event($crate::components::vector::VectorSplitter::<f64, f64, f64, 5>::update_state, notif_meta, a.clone()) },

                    $ComponentModelAddress::VectorStockVector3(a) => { Ok(()) },
                    $ComponentModelAddress::VectorProcessVector3(a) => { simu.process_event($crate::components::vector::VectorProcess::<Vector3, Vector3, f64>::update_state, notif_meta, a.clone()) },
                    $ComponentModelAddress::VectorSourceVector3(a) => { simu.process_event($crate::components::vector::VectorSource::<Vector3, Vector3>::update_state, notif_meta, a.clone()) },
                    $ComponentModelAddress::VectorSinkVector3(a) => { simu.process_event($crate::components::vector::VectorSink::<Vector3>::update_state, notif_meta, a.clone()) },
                    $ComponentModelAddress::VectorCombiner1Vector3(a) => { simu.process_event($crate::components::vector::VectorCombiner::<Vector3, Vector3, f64, 1>::update_state, notif_meta, a.clone()) },
                    $ComponentModelAddress::VectorCombiner2Vector3(a) => { simu.process_event($crate::components::vector::VectorCombiner::<Vector3, Vector3, f64, 2>::update_state, notif_meta, a.clone()) },
                    $ComponentModelAddress::VectorCombiner3Vector3(a) => { simu.process_event($crate::components::vector::VectorCombiner::<Vector3, Vector3, f64, 3>::update_state, notif_meta, a.clone()) },
                    $ComponentModelAddress::VectorCombiner4Vector3(a) => { simu.process_event($crate::components::vector::VectorCombiner::<Vector3, Vector3, f64, 4>::update_state, notif_meta, a.clone()) },
                    $ComponentModelAddress::VectorCombiner5Vector3(a) => { simu.process_event($crate::components::vector::VectorCombiner::<Vector3, Vector3, f64, 5>::update_state, notif_meta, a.clone()) },
                    $ComponentModelAddress::VectorSplitter1Vector3(a) => { simu.process_event($crate::components::vector::VectorSplitter::<Vector3, Vector3, f64, 1>::update_state, notif_meta, a.clone()) },
                    $ComponentModelAddress::VectorSplitter2Vector3(a) => { simu.process_event($crate::components::vector::VectorSplitter::<Vector3, Vector3, f64, 2>::update_state, notif_meta, a.clone()) },
                    $ComponentModelAddress::VectorSplitter3Vector3(a) => { simu.process_event($crate::components::vector::VectorSplitter::<Vector3, Vector3, f64, 3>::update_state, notif_meta, a.clone()) },
                    $ComponentModelAddress::VectorSplitter4Vector3(a) => { simu.process_event($crate::components::vector::VectorSplitter::<Vector3, Vector3, f64, 4>::update_state, notif_meta, a.clone()) },
                    $ComponentModelAddress::VectorSplitter5Vector3(a) => { simu.process_event($crate::components::vector::VectorSplitter::<Vector3, Vector3, f64, 5>::update_state, notif_meta, a.clone()) },

                    $ComponentModelAddress::DiscreteStockString(a) => { Ok(()) },
                    $ComponentModelAddress::DiscreteProcessString(a) => { simu.process_event($crate::components::discrete::DiscreteProcess::<Option<String>, (), Option<String>>::update_state, notif_meta, a.clone()) },
                    $ComponentModelAddress::DiscreteParallelProcessString(a) => { simu.process_event($crate::components::discrete::DiscreteParallelProcess::<Option<String>, (), Option<String>>::update_state, notif_meta, a.clone()) },
                    $ComponentModelAddress::DiscreteSourceString(a) => { simu.process_event($crate::components::discrete::DiscreteSource::<Option<String>, StringItemFactory>::update_state, notif_meta, a.clone()) },
                    $ComponentModelAddress::DiscreteSinkString(a) => { simu.process_event($crate::components::discrete::DiscreteSink::<Option<String>, ()>::update_state, notif_meta, a.clone()) },
                    a => {
                        <Self as CustomInit>::initialise(a, simu)
                    }
                }
            }
        }

        #[derive(Display)]
        pub enum $ScheduledEventConfig {
            SetLowCapacity(f64),
            SetMaxCapacity(f64),
            SetProcessQuantity(DistributionConfig),
            SetProcessTime(DistributionConfig),
        }

        impl $ScheduledEventConfig {
            pub fn schedule_event(self, time: $crate::nexosim::MonotonicTime, scheduler: &mut $crate::nexosim::Scheduler, addr: $ComponentModelAddress, df: &mut DistributionFactory) -> Result<(), Box<dyn ::std::error::Error>> {
                match (self, addr) {
                    ($ScheduledEventConfig::SetLowCapacity(low_capacity), $ComponentModelAddress::VectorStockF64(addr)) => {
                        scheduler.schedule_event(time, $crate::components::vector::VectorStock::<f64>::with_low_capacity_inplace, low_capacity, addr.clone())?;
                        scheduler.schedule_event(time, $crate::components::vector::VectorStock::<f64>::emit_change, NotificationMetadata {
                            time,
                            element_from: "Scheduler".into(),
                            message: "Low capacity change".into(),
                        }, addr.clone())?;
                        Ok(())
                    },
                    ($ScheduledEventConfig::SetProcessQuantity(distr), $ComponentModelAddress::VectorProcessF64(addr)) => {
                        let distr_instance = df.create(distr)?;
                        scheduler.schedule_event(time, $crate::components::vector::VectorProcess::<f64, f64, f64>::with_process_quantity_distr_inplace, distr_instance, addr.clone())?;
                        scheduler.schedule_event(time, $crate::components::vector::VectorProcess::<f64, f64, f64>::update_state, NotificationMetadata {
                            time,
                            element_from: "Scheduler".into(),
                            message: "Process quantity change".into(),
                        }, addr.clone())?;
                        Ok(())
                    }
                    (a, b) => {
                        Err(format!("schedule_event not implemented for types ({}, {})", a, b).into())
                    }
                }
            }
        }

        #[macro_export]
        macro_rules! connect_components {
            (&mut $a:ident, &mut $b:ident) => {
                $ComponentModel::connect_components(&mut $a, &mut $b, None)
            };
            (&mut $a:ident, &mut $b:ident, $n:expr) => {
                $ComponentModel::connect_components(&mut $a, &mut $b, Some($n))
            };
        }

        #[macro_export]
        macro_rules! connect_logger {
            (&mut $a:ident, &mut $b:ident) => {
                $ComponentLogger::connect_logger(&mut $a, &mut $b, None)
            };
            (&mut $a:ident, &mut $b:ident, $n:expr) => {
                $ComponentLogger::connect_logger(&mut $a, &mut $b, Some($n))
            };
        }

        macro_rules! register_component {
            ($sim_init:ident, $component:ident) => {
                $ComponentModel::register_component($sim_init, $component)
            };
        }

        macro_rules! create_scheduled_event {
            (&mut $scheduler:ident, $time:ident, $event_config:ident, $component_addr:ident, &mut $df:ident) => {
                $ScheduledEventConfig::schedule_event($event_config, $time, &mut $scheduler, $component_addr, &mut $df)
            }
        }
    }
}
