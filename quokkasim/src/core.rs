use std::{error::Error, fmt::Debug, fs::File, time::Duration};
use crate::common::{NotificationMetadata};
use csv::WriterBuilder;
use nexosim::{model::{Context, Model}, ports::EventQueue, simulation::ExecutionError};
use serde::Serialize;
use tai_time::MonotonicTime;

pub struct SubtractParts<T, U> {
    pub remaining: T,
    pub subtracted: U,
}

impl VectorArithmetic<f64, f64, f64> for f64 {
    fn add(&mut self, other: Self) {
        *self += other;
    }

    fn subtract_parts(&self, quantity: f64) -> SubtractParts<Self, f64> {
        SubtractParts { remaining: self - quantity, subtracted: quantity }
    }

    fn total(&self) -> f64 {
        *self
    }
}

#[derive(Debug, Clone)]
pub struct Vector3 {
    pub values: [f64; 3],
}

impl VectorArithmetic<Vector3, f64, f64> for Vector3 {
    fn add(&mut self, other: Vector3) {
        self.values[0] += other.values[0];
        self.values[1] += other.values[1];
        self.values[2] += other.values[2];
    }

    fn subtract_parts(&self, quantity: f64) -> SubtractParts<Self, Vector3> {
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
    fn add(&mut self, other: A);
    fn subtract_parts(&self, quantity: B) -> SubtractParts<Self, A>;
    fn total(&self) -> C;
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

        $(#[$model_init_meta:meta])*
        pub enum $ComponentInit:ident {}

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
            SequenceStockString($crate::components::sequence::SequenceStock<String>, $crate::nexosim::Mailbox<$crate::components::sequence::SequenceStock<String>>),
            SequenceProcessString($crate::components::sequence::SequenceProcess<Option<String>, (), Option<String>>, $crate::nexosim::Mailbox<$crate::components::sequence::SequenceProcess<Option<String>, (), Option<String>>>),
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


                    ($ComponentModel::SequenceStockString(a, ad), $ComponentModel::SequenceProcessString(b, bd), _) => {
                        a.state_emitter.connect($crate::components::sequence::SequenceProcess::update_state, bd.address());
                        b.req_upstream.connect($crate::components::sequence::SequenceStock::get_state_async, ad.address());
                        b.withdraw_upstream.connect($crate::components::sequence::SequenceStock::remove, ad.address());
                        Ok(())
                    },
                    ($ComponentModel::SequenceProcessString(a, ad), $ComponentModel::SequenceStockString(b, bd), _) => {
                        b.state_emitter.connect($crate::components::sequence::SequenceProcess::update_state, ad.address());
                        a.req_downstream.connect($crate::components::sequence::SequenceStock::get_state_async, bd.address());
                        a.push_downstream.connect($crate::components::sequence::SequenceStock::add, bd.address());
                        Ok(())
                    },
                    (a,b,n) => {
                        <$ComponentModel as CustomComponentConnection>::connect_components(a, b, n)
                    }
                }
            }

            pub fn register_component(mut sim_init: $crate::nexosim::SimInit, mut init_configs: &mut Vec<$ComponentInit>, component: Self) -> $crate::nexosim::SimInit {
                
                match component {
                    $ComponentModel::VectorProcessF64(a, mb) => {
                        let name = a.element_name.clone();
                        let addr = mb.address();
                        sim_init = sim_init.add_model(a, mb, name);
                        init_configs.push($ComponentInit::VectorProcessF64(addr));
                    },
                    $ComponentModel::VectorStockF64(a, mb) => {
                        let name = a.element_name.clone();
                        let addr = mb.address();
                        sim_init = sim_init.add_model(a, mb, name);
                        init_configs.push($ComponentInit::VectorStockF64(addr));
                    },
                    $ComponentModel::VectorStockVector3(a, mb) => {
                        let name = a.element_name.clone();
                        let addr = mb.address();
                        sim_init = sim_init.add_model(a, mb, name);
                        init_configs.push($ComponentInit::VectorStockVector3(addr));
                    },
                    $ComponentModel::VectorProcessVector3(a, mb) => {
                        let name = a.element_name.clone();
                        let addr = mb.address();
                        sim_init = sim_init.add_model(a, mb, name);
                        init_configs.push($ComponentInit::VectorProcessVector3(addr));
                    },
                    $ComponentModel::VectorCombiner1Vector3(a, mb) => {
                        let name = a.element_name.clone();
                        let addr = mb.address();
                        sim_init = sim_init.add_model(a, mb, name);
                        init_configs.push($ComponentInit::VectorCombiner1Vector3(addr));
                    },
                    $ComponentModel::VectorCombiner2Vector3(a, mb) => {
                        let name = a.element_name.clone();
                        let addr = mb.address();
                        sim_init = sim_init.add_model(a, mb, name);
                        init_configs.push($ComponentInit::VectorCombiner2Vector3(addr));
                    },
                    $ComponentModel::VectorCombiner3Vector3(a, mb) => {
                        let name = a.element_name.clone();
                        let addr = mb.address();
                        sim_init = sim_init.add_model(a, mb, name);
                        init_configs.push($ComponentInit::VectorCombiner3Vector3(addr));
                    },
                    $ComponentModel::VectorCombiner4Vector3(a, mb) => {
                        let name = a.element_name.clone();
                        let addr = mb.address();
                        sim_init = sim_init.add_model(a, mb, name);
                        init_configs.push($ComponentInit::VectorCombiner4Vector3(addr));
                    },
                    $ComponentModel::VectorCombiner5Vector3(a, mb) => {
                        let name = a.element_name.clone();
                        let addr = mb.address();
                        sim_init = sim_init.add_model(a, mb, name);
                        init_configs.push($ComponentInit::VectorCombiner5Vector3(addr));
                    },
                    $ComponentModel::VectorSplitter1Vector3(a, mb) => {
                        let name = a.element_name.clone();
                        let addr = mb.address();
                        sim_init = sim_init.add_model(a, mb, name);
                        init_configs.push($ComponentInit::VectorSplitter1Vector3(addr));
                    },
                    $ComponentModel::VectorSplitter2Vector3(a, mb) => {
                        let name = a.element_name.clone();
                        let addr = mb.address();
                        sim_init = sim_init.add_model(a, mb, name);
                        init_configs.push($ComponentInit::VectorSplitter2Vector3(addr));
                    },
                    $ComponentModel::VectorSplitter3Vector3(a, mb) => {
                        let name = a.element_name.clone();
                        let addr = mb.address();
                        sim_init = sim_init.add_model(a, mb, name);
                        init_configs.push($ComponentInit::VectorSplitter3Vector3(addr));
                    },
                    $ComponentModel::VectorSplitter4Vector3(a, mb) => {
                        let name = a.element_name.clone();
                        let addr = mb.address();
                        sim_init = sim_init.add_model(a, mb, name);
                        init_configs.push($ComponentInit::VectorSplitter4Vector3(addr));
                    },
                    $ComponentModel::VectorSplitter5Vector3(a, mb) => {
                        let name = a.element_name.clone();
                        let addr = mb.address();
                        sim_init = sim_init.add_model(a, mb, name);
                        init_configs.push($ComponentInit::VectorSplitter5Vector3(addr));
                    },
                    $ComponentModel::SequenceStockString(a, mb) => {
                        let name = a.element_name.clone();
                        let addr = mb.address();
                        sim_init = sim_init.add_model(a, mb, name);
                        init_configs.push($ComponentInit::SequenceStockString(addr));
                    },
                    $ComponentModel::SequenceProcessString(a, mb) => {
                        let name = a.element_name.clone();
                        let addr = mb.address();
                        sim_init = sim_init.add_model(a, mb, name);
                        init_configs.push($ComponentInit::SequenceProcessString(addr));
                    },
                    $(
                        $ComponentModel::$R(a, mb) => {
                            let name = a.element_name.clone();
                            let addr = mb.address();
                            sim_init = sim_init.add_model(a, mb, name);
                            init_configs.push($ComponentInit::$R(addr));
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
                    $ComponentModel::SequenceStockString(_, mb) => $ComponentModelAddress::SequenceStockString(mb.address()),
                    $ComponentModel::SequenceProcessString(_, mb) => $ComponentModelAddress::SequenceProcessString(mb.address()),
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
            SequenceStockString($crate::nexosim::Address<$crate::components::sequence::SequenceStock<String>>),
            SequenceProcessString($crate::nexosim::Address<$crate::components::sequence::SequenceProcess<Option<String>, (), Option<String>>>),
        }

        $(#[$logger_enum_meta])*
        #[derive(Display)]
        pub enum $ComponentLogger {
            VectorStockLoggerF64($crate::components::vector::VectorStockLogger<f64>),
            VectorProcessLoggerF64($crate::components::vector::VectorProcessLogger<f64>),

            VectorStockLoggerVector3($crate::components::vector::VectorStockLogger<Vector3>),
            VectorProcessLoggerVector3($crate::components::vector::VectorProcessLogger<Vector3>),

            SequenceStockLoggerString($crate::components::sequence::SequenceStockLogger<String>),
            SequenceProcessLoggerString($crate::components::sequence::SequenceProcessLogger<Option<String>>),
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
                    ($ComponentLogger::VectorStockLoggerVector3(a), $ComponentModel::VectorStockVector3(b, bd), _) => {
                        b.log_emitter.connect_sink(&a.buffer);
                        Ok(())
                    },
                    ($ComponentLogger::VectorProcessLoggerVector3(a), $ComponentModel::VectorProcessVector3(b, bd), _) => {
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

                    ($ComponentLogger::SequenceStockLoggerString(a), $ComponentModel::SequenceStockString(b, bd), _) => {
                        b.log_emitter.connect_sink(&a.buffer);
                        Ok(())
                    },
                    ($ComponentLogger::SequenceProcessLoggerString(a), $ComponentModel::SequenceProcessString(b, bd), _) => {
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

                    $ComponentLogger::SequenceStockLoggerString(a) => { a.write_csv(dir.to_string()) },
                    $ComponentLogger::SequenceProcessLoggerString(a) => { a.write_csv(dir.to_string()) },

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

        pub enum $ComponentInit {
            VectorStockF64($crate::nexosim::Address<$crate::components::vector::VectorStock<f64>>),
            VectorProcessF64($crate::nexosim::Address<$crate::components::vector::VectorProcess<f64, f64, f64>>),
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

            SequenceStockString($crate::nexosim::Address<$crate::components::sequence::SequenceStock<String>>),
            SequenceProcessString($crate::nexosim::Address<$crate::components::sequence::SequenceProcess<Option<String>, (), Option<String>>>),
            $(
                $R $( ( $crate::nexosim::Address<$RT> ) )?
            ),*
        }

        impl $ComponentInit {
            fn initialise(&mut self, simu: &mut $crate::nexosim::Simulation) -> Result<(), $crate::nexosim::ExecutionError> {
                let notif_meta = NotificationMetadata {
                    time: simu.time(),
                    element_from: "Init".into(),
                    message: "Init".into(),
                }; 
                match self {
                    $ComponentInit::VectorStockF64(a) => { Ok(()) },
                    $ComponentInit::VectorProcessF64(a) => { simu.process_event($crate::components::vector::VectorProcess::<f64, f64, f64>::update_state, notif_meta, a.clone()) },
                    $ComponentInit::VectorCombiner1F64(a) => { simu.process_event($crate::components::vector::VectorCombiner::<f64, f64, f64, 1>::update_state, notif_meta, a.clone()) },
                    $ComponentInit::VectorCombiner2F64(a) => { simu.process_event($crate::components::vector::VectorCombiner::<f64, f64, f64, 2>::update_state, notif_meta, a.clone()) },
                    $ComponentInit::VectorCombiner3F64(a) => { simu.process_event($crate::components::vector::VectorCombiner::<f64, f64, f64, 3>::update_state, notif_meta, a.clone()) },
                    $ComponentInit::VectorCombiner4F64(a) => { simu.process_event($crate::components::vector::VectorCombiner::<f64, f64, f64, 4>::update_state, notif_meta, a.clone()) },
                    $ComponentInit::VectorCombiner5F64(a) => { simu.process_event($crate::components::vector::VectorCombiner::<f64, f64, f64, 5>::update_state, notif_meta, a.clone()) },
                    $ComponentInit::VectorSplitter1F64(a) => { simu.process_event($crate::components::vector::VectorSplitter::<f64, f64, f64, 1>::update_state, notif_meta, a.clone()) },
                    $ComponentInit::VectorSplitter2F64(a) => { simu.process_event($crate::components::vector::VectorSplitter::<f64, f64, f64, 2>::update_state, notif_meta, a.clone()) },
                    $ComponentInit::VectorSplitter3F64(a) => { simu.process_event($crate::components::vector::VectorSplitter::<f64, f64, f64, 3>::update_state, notif_meta, a.clone()) },
                    $ComponentInit::VectorSplitter4F64(a) => { simu.process_event($crate::components::vector::VectorSplitter::<f64, f64, f64, 4>::update_state, notif_meta, a.clone()) },
                    $ComponentInit::VectorSplitter5F64(a) => { simu.process_event($crate::components::vector::VectorSplitter::<f64, f64, f64, 5>::update_state, notif_meta, a.clone()) },

                    $ComponentInit::VectorStockVector3(a) => { Ok(()) },
                    $ComponentInit::VectorProcessVector3(a) => { simu.process_event($crate::components::vector::VectorProcess::<Vector3, Vector3, f64>::update_state, notif_meta, a.clone()) },
                    $ComponentInit::VectorCombiner1Vector3(a) => { simu.process_event($crate::components::vector::VectorCombiner::<Vector3, Vector3, f64, 1>::update_state, notif_meta, a.clone()) },
                    $ComponentInit::VectorCombiner2Vector3(a) => { simu.process_event($crate::components::vector::VectorCombiner::<Vector3, Vector3, f64, 2>::update_state, notif_meta, a.clone()) },
                    $ComponentInit::VectorCombiner3Vector3(a) => { simu.process_event($crate::components::vector::VectorCombiner::<Vector3, Vector3, f64, 3>::update_state, notif_meta, a.clone()) },
                    $ComponentInit::VectorCombiner4Vector3(a) => { simu.process_event($crate::components::vector::VectorCombiner::<Vector3, Vector3, f64, 4>::update_state, notif_meta, a.clone()) },
                    $ComponentInit::VectorCombiner5Vector3(a) => { simu.process_event($crate::components::vector::VectorCombiner::<Vector3, Vector3, f64, 5>::update_state, notif_meta, a.clone()) },
                    $ComponentInit::VectorSplitter1Vector3(a) => { simu.process_event($crate::components::vector::VectorSplitter::<Vector3, Vector3, f64, 1>::update_state, notif_meta, a.clone()) },
                    $ComponentInit::VectorSplitter2Vector3(a) => { simu.process_event($crate::components::vector::VectorSplitter::<Vector3, Vector3, f64, 2>::update_state, notif_meta, a.clone()) },
                    $ComponentInit::VectorSplitter3Vector3(a) => { simu.process_event($crate::components::vector::VectorSplitter::<Vector3, Vector3, f64, 3>::update_state, notif_meta, a.clone()) },
                    $ComponentInit::VectorSplitter4Vector3(a) => { simu.process_event($crate::components::vector::VectorSplitter::<Vector3, Vector3, f64, 4>::update_state, notif_meta, a.clone()) },
                    $ComponentInit::VectorSplitter5Vector3(a) => { simu.process_event($crate::components::vector::VectorSplitter::<Vector3, Vector3, f64, 5>::update_state, notif_meta, a.clone()) },

                    $ComponentInit::SequenceStockString(a) => { Ok(()) },
                    $ComponentInit::SequenceProcessString(a) => { simu.process_event($crate::components::sequence::SequenceProcess::<Option<String>, (), Option<String>>::update_state, notif_meta, a.clone()) },
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
            ($sim_init:ident, &mut $init_configs:ident, $component:ident) => {
                $ComponentModel::register_component($sim_init, &mut $init_configs, $component)
            };
        }

        macro_rules! create_scheduled_event {
            (&mut $scheduler:ident, $time:ident, $event_config:ident, $component_addr:ident, &mut $df:ident) => {
                $ScheduledEventConfig::schedule_event($event_config, $time, &mut $scheduler, $component_addr, &mut $df)
            }
        }
    }
}
