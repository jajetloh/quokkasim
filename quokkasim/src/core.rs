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
        pub enum $ComponentsName:ident {
          $(
            $(#[$components_var_meta:meta])*
            $R:ident $( ( $RT:ty, $RT2:ty ) )?
          ),* $(,)?
        }

        $(#[$components_address_enum_meta:meta])*
        pub enum $ComponentsAddressName:ident {
          $(
            $(#[$components_address_var_meta:meta])*
            $Q:ident $( ( $QT:ty ) )?
          ),* $(,)?
        }

        $(#[$logger_enum_meta:meta])*
        pub enum $LoggersName:ident {
            $(
                $(#[$logger_var_meta:meta])*
                $U:ident $( ( $UT:ty ) )?
            ),* $(,)?
        }

        $(#[$model_init_meta:meta])*
        pub enum $ModelInitName:ident {}

        $(#[$sch_event_config_enum_meta:meta])*
        pub enum $ScheduledEventConfig:ident {
            $(
                $(#[$sch_event_config_meta:meta])*
                $W:ident $( ( $WT:ty ) )?
            ),* $(,)?
        }

        $(#[$sch_event_enum_meta:meta])*
        pub enum $ScheduledEvent:ident {
            $(
                $(#[$sch_event_meta:meta])*
                $X:ident $( ( $XT:ty ) )?
            ),* $(,)?
        }
    ) => {

        use ::quokkasim::strum_macros::Display;
        use ::quokkasim::core::Logger;

        $(#[$components_enum_meta])*
        #[derive(Display)]
        pub enum $ComponentsName {
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
  
        impl $ComponentsName {
            pub fn connect_components(
                mut a: &mut $ComponentsName,
                mut b: &mut $ComponentsName,
                n: Option<usize>,
            ) -> Result<(), Box<dyn ::std::error::Error>>{
                use $crate::core::CustomComponentConnection;
                match (a, b, n) {
                    ($ComponentsName::VectorStockF64(a, ad), $ComponentsName::VectorProcessF64(b, bd), _) => {
                        a.state_emitter.connect($crate::components::vector::VectorProcess::update_state, bd.address());
                        b.req_upstream.connect($crate::components::vector::VectorStock::get_state_async, ad.address());
                        b.withdraw_upstream.connect($crate::components::vector::VectorStock::remove, ad.address());
                        Ok(())
                    },
                    ($ComponentsName::VectorProcessF64(a, ad), $ComponentsName::VectorStockF64(b, bd), _) => {
                        b.state_emitter.connect($crate::components::vector::VectorProcess::update_state, ad.address());
                        a.req_downstream.connect($crate::components::vector::VectorStock::get_state_async, bd.address());
                        a.push_downstream.connect($crate::components::vector::VectorStock::add, bd.address());
                        Ok(())
                    },
                    ($ComponentsName::VectorStockVector3(a, ad), $ComponentsName::VectorProcessVector3(b, bd), _) => {
                        a.state_emitter.connect($crate::components::vector::VectorProcess::update_state, bd.address());
                        b.req_upstream.connect($crate::components::vector::VectorStock::get_state_async, ad.address());
                        b.withdraw_upstream.connect($crate::components::vector::VectorStock::remove, ad.address());
                        Ok(())
                    },
                    ($ComponentsName::VectorProcessVector3(a, ad), $ComponentsName::VectorStockVector3(b, bd), _) => {
                        b.state_emitter.connect($crate::components::vector::VectorProcess::update_state, ad.address());
                        a.req_downstream.connect($crate::components::vector::VectorStock::get_state_async, bd.address());
                        a.push_downstream.connect($crate::components::vector::VectorStock::add, bd.address());
                        Ok(())
                    },

                    // VectorCombiner1Vector3
                    ($ComponentsName::VectorStockVector3(a, am), $ComponentsName::VectorCombiner1Vector3(b, bm), Some(n)) => {
                        a.state_emitter.connect($crate::components::vector::VectorCombiner::update_state, bm.address());
                        b.req_upstreams[n].connect($crate::components::vector::VectorStock::get_state_async, am.address());
                        b.withdraw_upstreams[n].connect($crate::components::vector::VectorStock::remove, am.address());
                        Ok(())
                    },
                    ($ComponentsName::VectorCombiner1Vector3(a, am), $ComponentsName::VectorStockVector3(b, bm), _) => {
                        b.state_emitter.connect($crate::components::vector::VectorCombiner::update_state, am.address());
                        a.req_downstream.connect($crate::components::vector::VectorStock::get_state_async, bm.address());
                        a.push_downstream.connect($crate::components::vector::VectorStock::add, bm.address());
                        Ok(())
                    },
                    // VectorCombiner2Vector3
                    ($ComponentsName::VectorStockVector3(a, am), $ComponentsName::VectorCombiner2Vector3(b, bm), Some(n)) => {
                        a.state_emitter.connect($crate::components::vector::VectorCombiner::update_state, bm.address());
                        b.req_upstreams[n].connect($crate::components::vector::VectorStock::get_state_async, am.address());
                        b.withdraw_upstreams[n].connect($crate::components::vector::VectorStock::remove, am.address());
                        Ok(())
                    },
                    ($ComponentsName::VectorCombiner2Vector3(a, am), $ComponentsName::VectorStockVector3(b, bm), _) => {
                        b.state_emitter.connect($crate::components::vector::VectorCombiner::update_state, am.address());
                        a.req_downstream.connect($crate::components::vector::VectorStock::get_state_async, bm.address());
                        a.push_downstream.connect($crate::components::vector::VectorStock::add, bm.address());
                        Ok(())
                    },
                    // VectorCombiner3Vector3
                    ($ComponentsName::VectorStockVector3(a, am), $ComponentsName::VectorCombiner3Vector3(b, bm), Some(n)) => {
                        a.state_emitter.connect($crate::components::vector::VectorCombiner::update_state, bm.address());
                        b.req_upstreams[n].connect($crate::components::vector::VectorStock::get_state_async, am.address());
                        b.withdraw_upstreams[n].connect($crate::components::vector::VectorStock::remove, am.address());
                        Ok(())
                    },
                    ($ComponentsName::VectorCombiner3Vector3(a, am), $ComponentsName::VectorStockVector3(b, bm), _) => {
                        b.state_emitter.connect($crate::components::vector::VectorCombiner::update_state, am.address());
                        a.req_downstream.connect($crate::components::vector::VectorStock::get_state_async, bm.address());
                        a.push_downstream.connect($crate::components::vector::VectorStock::add, bm.address());
                        Ok(())
                    },
                    // VectorCombiner4Vector3
                    ($ComponentsName::VectorStockVector3(a, am), $ComponentsName::VectorCombiner4Vector3(b, bm), Some(n)) => {
                        a.state_emitter.connect($crate::components::vector::VectorCombiner::update_state, bm.address());
                        b.req_upstreams[n].connect($crate::components::vector::VectorStock::get_state_async, am.address());
                        b.withdraw_upstreams[n].connect($crate::components::vector::VectorStock::remove, am.address());
                        Ok(())
                    },
                    ($ComponentsName::VectorCombiner4Vector3(a, am), $ComponentsName::VectorStockVector3(b, bm), _) => {
                        b.state_emitter.connect($crate::components::vector::VectorCombiner::update_state, am.address());
                        a.req_downstream.connect($crate::components::vector::VectorStock::get_state_async, bm.address());
                        a.push_downstream.connect($crate::components::vector::VectorStock::add, bm.address());
                        Ok(())
                    },
                    // VectorCombiner5Vector3
                    ($ComponentsName::VectorStockVector3(a, am), $ComponentsName::VectorCombiner5Vector3(b, bm), Some(n)) => {
                        a.state_emitter.connect($crate::components::vector::VectorCombiner::update_state, bm.address());
                        b.req_upstreams[n].connect($crate::components::vector::VectorStock::get_state_async, am.address());
                        b.withdraw_upstreams[n].connect($crate::components::vector::VectorStock::remove, am.address());
                        Ok(())
                    },
                    ($ComponentsName::VectorCombiner5Vector3(a, am), $ComponentsName::VectorStockVector3(b, bm), _) => {
                        b.state_emitter.connect($crate::components::vector::VectorCombiner::update_state, am.address());
                        a.req_downstream.connect($crate::components::vector::VectorStock::get_state_async, bm.address());
                        a.push_downstream.connect($crate::components::vector::VectorStock::add, bm.address());
                        Ok(())
                    },


                    // VectorSplitter1Vector3
                    ($ComponentsName::VectorStockVector3(a, amb), $ComponentsName::VectorSplitter1Vector3(b, bmb), _) => {
                        a.state_emitter.connect($crate::components::vector::VectorSplitter::update_state, bmb.address());
                        b.req_upstream.connect($crate::components::vector::VectorStock::get_state_async, amb.address());
                        b.withdraw_upstream.connect($crate::components::vector::VectorStock::remove, amb.address());
                        Ok(())
                    },
                    ($ComponentsName::VectorSplitter1Vector3(a, amb), $ComponentsName::VectorStockVector3(b, bmb), Some(n)) => {
                        b.state_emitter.connect($crate::components::vector::VectorSplitter::update_state, amb.address());
                        a.req_downstreams[n].connect($crate::components::vector::VectorStock::get_state_async, bmb.address());
                        a.push_downstreams[n].connect($crate::components::vector::VectorStock::add, bmb.address());
                        Ok(())
                    },
                    // VectorSplitter2Vector3
                    ($ComponentsName::VectorStockVector3(a, amb), $ComponentsName::VectorSplitter2Vector3(b, bmb), _) => {
                        a.state_emitter.connect($crate::components::vector::VectorSplitter::update_state, bmb.address());
                        b.req_upstream.connect($crate::components::vector::VectorStock::get_state_async, amb.address());
                        b.withdraw_upstream.connect($crate::components::vector::VectorStock::remove, amb.address());
                        Ok(())
                    },
                    ($ComponentsName::VectorSplitter2Vector3(a, amb), $ComponentsName::VectorStockVector3(b, bmb), Some(n)) => {
                        b.state_emitter.connect($crate::components::vector::VectorSplitter::update_state, amb.address());
                        a.req_downstreams[n].connect($crate::components::vector::VectorStock::get_state_async, bmb.address());
                        a.push_downstreams[n].connect($crate::components::vector::VectorStock::add, bmb.address());
                        Ok(())
                    },
                    // VectorSplitter3Vector3
                    ($ComponentsName::VectorStockVector3(a, amb), $ComponentsName::VectorSplitter3Vector3(b, bmb), _) => {
                        a.state_emitter.connect($crate::components::vector::VectorSplitter::update_state, bmb.address());
                        b.req_upstream.connect($crate::components::vector::VectorStock::get_state_async, amb.address());
                        b.withdraw_upstream.connect($crate::components::vector::VectorStock::remove, amb.address());
                        Ok(())
                    },
                    ($ComponentsName::VectorSplitter3Vector3(a, amb), $ComponentsName::VectorStockVector3(b, bmb), Some(n)) => {
                        b.state_emitter.connect($crate::components::vector::VectorSplitter::update_state, amb.address());
                        a.req_downstreams[n].connect($crate::components::vector::VectorStock::get_state_async, bmb.address());
                        a.push_downstreams[n].connect($crate::components::vector::VectorStock::add, bmb.address());
                        Ok(())
                    },
                    // VectorSplitter4Vector3
                    ($ComponentsName::VectorStockVector3(a, amb), $ComponentsName::VectorSplitter4Vector3(b, bmb), _) => {
                        a.state_emitter.connect($crate::components::vector::VectorSplitter::update_state, bmb.address());
                        b.req_upstream.connect($crate::components::vector::VectorStock::get_state_async, amb.address());
                        b.withdraw_upstream.connect($crate::components::vector::VectorStock::remove, amb.address());
                        Ok(())
                    },
                    ($ComponentsName::VectorSplitter4Vector3(a, amb), $ComponentsName::VectorStockVector3(b, bmb), Some(n)) => {
                        b.state_emitter.connect($crate::components::vector::VectorSplitter::update_state, amb.address());
                        a.req_downstreams[n].connect($crate::components::vector::VectorStock::get_state_async, bmb.address());
                        a.push_downstreams[n].connect($crate::components::vector::VectorStock::add, bmb.address());
                        Ok(())
                    },
                    // VectorSplitter5Vector3
                    ($ComponentsName::VectorStockVector3(a, amb), $ComponentsName::VectorSplitter5Vector3(b, bmb), _) => {
                        a.state_emitter.connect($crate::components::vector::VectorSplitter::update_state, bmb.address());
                        b.req_upstream.connect($crate::components::vector::VectorStock::get_state_async, amb.address());
                        b.withdraw_upstream.connect($crate::components::vector::VectorStock::remove, amb.address());
                        Ok(())
                    },
                    ($ComponentsName::VectorSplitter5Vector3(a, amb), $ComponentsName::VectorStockVector3(b, bmb), Some(n)) => {
                        b.state_emitter.connect($crate::components::vector::VectorSplitter::update_state, amb.address());
                        a.req_downstreams[n].connect($crate::components::vector::VectorStock::get_state_async, bmb.address());
                        a.push_downstreams[n].connect($crate::components::vector::VectorStock::add, bmb.address());
                        Ok(())
                    },


                    ($ComponentsName::SequenceStockString(a, ad), $ComponentsName::SequenceProcessString(b, bd), _) => {
                        a.state_emitter.connect($crate::components::sequence::SequenceProcess::update_state, bd.address());
                        b.req_upstream.connect($crate::components::sequence::SequenceStock::get_state_async, ad.address());
                        b.withdraw_upstream.connect($crate::components::sequence::SequenceStock::remove, ad.address());
                        Ok(())
                    },
                    ($ComponentsName::SequenceProcessString(a, ad), $ComponentsName::SequenceStockString(b, bd), _) => {
                        b.state_emitter.connect($crate::components::sequence::SequenceProcess::update_state, ad.address());
                        a.req_downstream.connect($crate::components::sequence::SequenceStock::get_state_async, bd.address());
                        a.push_downstream.connect($crate::components::sequence::SequenceStock::add, bd.address());
                        Ok(())
                    },
                    (a,b,n) => {
                        <$ComponentsName as CustomComponentConnection>::connect_components(a, b, n)
                    }
                }
            }

            pub fn register_component(mut sim_init: $crate::nexosim::SimInit, mut init_configs: &mut Vec<$ModelInitName>, component: Self) -> $crate::nexosim::SimInit {
                
                match component {
                    $ComponentsName::VectorProcessF64(a, mb) => {
                        let name = a.element_name.clone();
                        let addr = mb.address();
                        sim_init = sim_init.add_model(a, mb, name);
                        init_configs.push($ModelInitName::VectorProcessF64(addr));
                    },
                    $ComponentsName::VectorStockF64(a, mb) => {
                        let name = a.element_name.clone();
                        let addr = mb.address();
                        sim_init = sim_init.add_model(a, mb, name);
                        init_configs.push($ModelInitName::VectorStockF64(addr));
                    },
                    $ComponentsName::VectorStockVector3(a, mb) => {
                        let name = a.element_name.clone();
                        let addr = mb.address();
                        sim_init = sim_init.add_model(a, mb, name);
                        init_configs.push($ModelInitName::VectorStockVector3(addr));
                    },
                    $ComponentsName::VectorProcessVector3(a, mb) => {
                        let name = a.element_name.clone();
                        let addr = mb.address();
                        sim_init = sim_init.add_model(a, mb, name);
                        init_configs.push($ModelInitName::VectorProcessVector3(addr));
                    },
                    $ComponentsName::VectorCombiner1Vector3(a, mb) => {
                        let name = a.element_name.clone();
                        let addr = mb.address();
                        sim_init = sim_init.add_model(a, mb, name);
                        init_configs.push($ModelInitName::VectorCombiner1Vector3(addr));
                    },
                    $ComponentsName::VectorCombiner2Vector3(a, mb) => {
                        let name = a.element_name.clone();
                        let addr = mb.address();
                        sim_init = sim_init.add_model(a, mb, name);
                        init_configs.push($ModelInitName::VectorCombiner2Vector3(addr));
                    },
                    $ComponentsName::VectorCombiner3Vector3(a, mb) => {
                        let name = a.element_name.clone();
                        let addr = mb.address();
                        sim_init = sim_init.add_model(a, mb, name);
                        init_configs.push($ModelInitName::VectorCombiner3Vector3(addr));
                    },
                    $ComponentsName::VectorCombiner4Vector3(a, mb) => {
                        let name = a.element_name.clone();
                        let addr = mb.address();
                        sim_init = sim_init.add_model(a, mb, name);
                        init_configs.push($ModelInitName::VectorCombiner4Vector3(addr));
                    },
                    $ComponentsName::VectorCombiner5Vector3(a, mb) => {
                        let name = a.element_name.clone();
                        let addr = mb.address();
                        sim_init = sim_init.add_model(a, mb, name);
                        init_configs.push($ModelInitName::VectorCombiner5Vector3(addr));
                    },
                    $ComponentsName::VectorSplitter1Vector3(a, mb) => {
                        let name = a.element_name.clone();
                        let addr = mb.address();
                        sim_init = sim_init.add_model(a, mb, name);
                        init_configs.push($ModelInitName::VectorSplitter1Vector3(addr));
                    },
                    $ComponentsName::VectorSplitter2Vector3(a, mb) => {
                        let name = a.element_name.clone();
                        let addr = mb.address();
                        sim_init = sim_init.add_model(a, mb, name);
                        init_configs.push($ModelInitName::VectorSplitter2Vector3(addr));
                    },
                    $ComponentsName::VectorSplitter3Vector3(a, mb) => {
                        let name = a.element_name.clone();
                        let addr = mb.address();
                        sim_init = sim_init.add_model(a, mb, name);
                        init_configs.push($ModelInitName::VectorSplitter3Vector3(addr));
                    },
                    $ComponentsName::VectorSplitter4Vector3(a, mb) => {
                        let name = a.element_name.clone();
                        let addr = mb.address();
                        sim_init = sim_init.add_model(a, mb, name);
                        init_configs.push($ModelInitName::VectorSplitter4Vector3(addr));
                    },
                    $ComponentsName::VectorSplitter5Vector3(a, mb) => {
                        let name = a.element_name.clone();
                        let addr = mb.address();
                        sim_init = sim_init.add_model(a, mb, name);
                        init_configs.push($ModelInitName::VectorSplitter5Vector3(addr));
                    },
                    $ComponentsName::SequenceStockString(a, mb) => {
                        let name = a.element_name.clone();
                        let addr = mb.address();
                        sim_init = sim_init.add_model(a, mb, name);
                        init_configs.push($ModelInitName::SequenceStockString(addr));
                    },
                    $ComponentsName::SequenceProcessString(a, mb) => {
                        let name = a.element_name.clone();
                        let addr = mb.address();
                        sim_init = sim_init.add_model(a, mb, name);
                        init_configs.push($ModelInitName::SequenceProcessString(addr));
                    },
                    $(
                        $ComponentsName::$R(a, mb) => {
                            let name = a.element_name.clone();
                            let addr = mb.address();
                            sim_init = sim_init.add_model(a, mb, name);
                            init_configs.push($ModelInitName::$R(addr));
                        }
                    ),*
                    _ => {
                        panic!("Component type {} not implemented for registration", component);
                    }
                };
                sim_init
            }

            pub fn get_address(&self) -> $ComponentsAddressName {
                match self {
                    $ComponentsName::VectorStockF64(_, mb) => $ComponentsAddressName::VectorStockF64(mb.address()),
                    $ComponentsName::VectorProcessF64(_, mb) => $ComponentsAddressName::VectorProcessF64(mb.address()),
                    x => {
                        panic!("Component type {} not implemented for address retrieval", x);
                    }
                }
            }

            // pub fn create_scheduled_event(self, time: $crate::core::MonotonicTime, event: $ModelScheduledEventName) -> $ModelScheduledEventName {
            //     use $crate::core::ScheduledEvent;
            //     match event {
            //         $(
            //             $(#[$sch_event_meta])*
            //             $W $( ( $WT ) )? => {
            //                 $crate::nexosim::ScheduledEvent::new(time, self, event)
            //             }
            //         ),*
            //         _ => {
            //             panic!("Scheduled event type {} not implemented for creation", event);
            //         }
            //     }

            // }
        }

        #[derive(Display)]
        pub enum $ComponentsAddressName {
            VectorStockF64($crate::nexosim::Address<$crate::components::vector::VectorStock<f64>>),
            VectorProcessF64($crate::nexosim::Address<$crate::components::vector::VectorProcess<f64, f64, f64>>),
            VectorStockVector3($crate::nexosim::Address<$crate::components::vector::VectorStock<Vector3>>),
            VectorProcessVector3($crate::nexosim::Address<$crate::components::vector::VectorProcess<Vector3, Vector3, f64>>),
        }

        $(#[$logger_enum_meta])*
        #[derive(Display)]
        pub enum $LoggersName {
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

        impl $LoggersName {
            pub fn connect_logger(a: &mut $LoggersName, b: &mut $ComponentsName, n: Option<usize>) -> Result<(), Box<dyn ::std::error::Error>> {
                use $crate::core::CustomLoggerConnection;
                match (a, b, n) {
                    ($LoggersName::VectorStockLoggerF64(a), $ComponentsName::VectorStockF64(b, bd), _) => {
                        b.log_emitter.connect_sink(&a.buffer);
                        Ok(())
                    },
                    ($LoggersName::VectorProcessLoggerF64(a), $ComponentsName::VectorProcessF64(b, bd), _) => {
                        b.log_emitter.connect_sink(&a.buffer);
                        Ok(())
                    },

                    ($LoggersName::VectorStockLoggerVector3(a), $ComponentsName::VectorStockVector3(b, bd), _) => {
                        b.log_emitter.connect_sink(&a.buffer);
                        Ok(())
                    },
                    ($LoggersName::VectorProcessLoggerVector3(a), $ComponentsName::VectorProcessVector3(b, bd), _) => {
                        b.log_emitter.connect_sink(&a.buffer);
                        Ok(())
                    },
                    ($LoggersName::VectorProcessLoggerVector3(a), $ComponentsName::VectorCombiner1Vector3(b, bmb), _) => {
                        b.log_emitter.connect_sink(&a.buffer);
                        Ok(())
                    },
                    ($LoggersName::VectorProcessLoggerVector3(a), $ComponentsName::VectorCombiner2Vector3(b, bmb), _) => {
                        b.log_emitter.connect_sink(&a.buffer);
                        Ok(())
                    },
                    ($LoggersName::VectorProcessLoggerVector3(a), $ComponentsName::VectorCombiner3Vector3(b, bmb), _) => {
                        b.log_emitter.connect_sink(&a.buffer);
                        Ok(())
                    },
                    ($LoggersName::VectorProcessLoggerVector3(a), $ComponentsName::VectorCombiner4Vector3(b, bmb), _) => {
                        b.log_emitter.connect_sink(&a.buffer);
                        Ok(())
                    },
                    ($LoggersName::VectorProcessLoggerVector3(a), $ComponentsName::VectorCombiner5Vector3(b, bmb), _) => {
                        b.log_emitter.connect_sink(&a.buffer);
                        Ok(())
                    },
                    ($LoggersName::VectorProcessLoggerVector3(a), $ComponentsName::VectorSplitter1Vector3(b, bmb), _) => {
                        b.log_emitter.connect_sink(&a.buffer);
                        Ok(())
                    },
                    ($LoggersName::VectorProcessLoggerVector3(a), $ComponentsName::VectorSplitter2Vector3(b, bmb), _) => {
                        b.log_emitter.connect_sink(&a.buffer);
                        Ok(())
                    },
                    ($LoggersName::VectorProcessLoggerVector3(a), $ComponentsName::VectorSplitter3Vector3(b, bmb), _) => {
                        b.log_emitter.connect_sink(&a.buffer);
                        Ok(())
                    },
                    ($LoggersName::VectorProcessLoggerVector3(a), $ComponentsName::VectorSplitter4Vector3(b, bmb), _) => {
                        b.log_emitter.connect_sink(&a.buffer);
                        Ok(())
                    },
                    ($LoggersName::VectorProcessLoggerVector3(a), $ComponentsName::VectorSplitter5Vector3(b, bmb), _) => {
                        b.log_emitter.connect_sink(&a.buffer);
                        Ok(())
                    },

                    ($LoggersName::SequenceStockLoggerString(a), $ComponentsName::SequenceStockString(b, bd), _) => {
                        b.log_emitter.connect_sink(&a.buffer);
                        Ok(())
                    },
                    ($LoggersName::SequenceProcessLoggerString(a), $ComponentsName::SequenceProcessString(b, bd), _) => {
                        b.log_emitter.connect_sink(&a.buffer);
                        Ok(())
                    },
                    (a,b,n) => <$LoggersName as CustomLoggerConnection>::connect_logger(a, b, n)
                }
            }

            pub fn write_csv(mut self, dir: &str) -> Result<(), Box<dyn Error>> {
                match self {
                    $LoggersName::VectorStockLoggerF64(a) => { a.write_csv(dir.to_string()) },
                    $LoggersName::VectorProcessLoggerF64(a) => { a.write_csv(dir.to_string()) },

                    $LoggersName::VectorStockLoggerVector3(a) => { a.write_csv(dir.to_string()) },
                    $LoggersName::VectorProcessLoggerVector3(a) => { a.write_csv(dir.to_string()) },

                    $LoggersName::SequenceStockLoggerString(a) => { a.write_csv(dir.to_string()) },
                    $LoggersName::SequenceProcessLoggerString(a) => { a.write_csv(dir.to_string()) },

                    $(
                        $LoggersName::$U (a) => {
                            a.write_csv(dir.to_string())
                        }
                    ),*
                    _ => {
                        Err(format!("Logger type {} not implemented for writing CSV", self).into())
                    }
                }
            } 
        }

        pub enum $ModelInitName {
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

        impl $ModelInitName {
            fn initialise(&mut self, simu: &mut $crate::nexosim::Simulation) -> Result<(), $crate::nexosim::ExecutionError> {
                let notif_meta = NotificationMetadata {
                    time: simu.time(),
                    element_from: "Init".into(),
                    message: "Init".into(),
                }; 
                match self {
                    $ModelInitName::VectorStockF64(a) => { Ok(()) },
                    $ModelInitName::VectorProcessF64(a) => { simu.process_event($crate::components::vector::VectorProcess::<f64, f64, f64>::update_state, notif_meta, a.clone()) },
                    $ModelInitName::VectorCombiner1F64(a) => { simu.process_event($crate::components::vector::VectorCombiner::<f64, f64, f64, 1>::update_state, notif_meta, a.clone()) },
                    $ModelInitName::VectorCombiner2F64(a) => { simu.process_event($crate::components::vector::VectorCombiner::<f64, f64, f64, 2>::update_state, notif_meta, a.clone()) },
                    $ModelInitName::VectorCombiner3F64(a) => { simu.process_event($crate::components::vector::VectorCombiner::<f64, f64, f64, 3>::update_state, notif_meta, a.clone()) },
                    $ModelInitName::VectorCombiner4F64(a) => { simu.process_event($crate::components::vector::VectorCombiner::<f64, f64, f64, 4>::update_state, notif_meta, a.clone()) },
                    $ModelInitName::VectorCombiner5F64(a) => { simu.process_event($crate::components::vector::VectorCombiner::<f64, f64, f64, 5>::update_state, notif_meta, a.clone()) },
                    $ModelInitName::VectorSplitter1F64(a) => { simu.process_event($crate::components::vector::VectorSplitter::<f64, f64, f64, 1>::update_state, notif_meta, a.clone()) },
                    $ModelInitName::VectorSplitter2F64(a) => { simu.process_event($crate::components::vector::VectorSplitter::<f64, f64, f64, 2>::update_state, notif_meta, a.clone()) },
                    $ModelInitName::VectorSplitter3F64(a) => { simu.process_event($crate::components::vector::VectorSplitter::<f64, f64, f64, 3>::update_state, notif_meta, a.clone()) },
                    $ModelInitName::VectorSplitter4F64(a) => { simu.process_event($crate::components::vector::VectorSplitter::<f64, f64, f64, 4>::update_state, notif_meta, a.clone()) },
                    $ModelInitName::VectorSplitter5F64(a) => { simu.process_event($crate::components::vector::VectorSplitter::<f64, f64, f64, 5>::update_state, notif_meta, a.clone()) },

                    $ModelInitName::VectorStockVector3(a) => { Ok(()) },
                    $ModelInitName::VectorProcessVector3(a) => { simu.process_event($crate::components::vector::VectorProcess::<Vector3, Vector3, f64>::update_state, notif_meta, a.clone()) },
                    $ModelInitName::VectorCombiner1Vector3(a) => { simu.process_event($crate::components::vector::VectorCombiner::<Vector3, Vector3, f64, 1>::update_state, notif_meta, a.clone()) },
                    $ModelInitName::VectorCombiner2Vector3(a) => { simu.process_event($crate::components::vector::VectorCombiner::<Vector3, Vector3, f64, 2>::update_state, notif_meta, a.clone()) },
                    $ModelInitName::VectorCombiner3Vector3(a) => { simu.process_event($crate::components::vector::VectorCombiner::<Vector3, Vector3, f64, 3>::update_state, notif_meta, a.clone()) },
                    $ModelInitName::VectorCombiner4Vector3(a) => { simu.process_event($crate::components::vector::VectorCombiner::<Vector3, Vector3, f64, 4>::update_state, notif_meta, a.clone()) },
                    $ModelInitName::VectorCombiner5Vector3(a) => { simu.process_event($crate::components::vector::VectorCombiner::<Vector3, Vector3, f64, 5>::update_state, notif_meta, a.clone()) },
                    $ModelInitName::VectorSplitter1Vector3(a) => { simu.process_event($crate::components::vector::VectorSplitter::<Vector3, Vector3, f64, 1>::update_state, notif_meta, a.clone()) },
                    $ModelInitName::VectorSplitter2Vector3(a) => { simu.process_event($crate::components::vector::VectorSplitter::<Vector3, Vector3, f64, 2>::update_state, notif_meta, a.clone()) },
                    $ModelInitName::VectorSplitter3Vector3(a) => { simu.process_event($crate::components::vector::VectorSplitter::<Vector3, Vector3, f64, 3>::update_state, notif_meta, a.clone()) },
                    $ModelInitName::VectorSplitter4Vector3(a) => { simu.process_event($crate::components::vector::VectorSplitter::<Vector3, Vector3, f64, 4>::update_state, notif_meta, a.clone()) },
                    $ModelInitName::VectorSplitter5Vector3(a) => { simu.process_event($crate::components::vector::VectorSplitter::<Vector3, Vector3, f64, 5>::update_state, notif_meta, a.clone()) },

                    $ModelInitName::SequenceStockString(a) => { Ok(()) },
                    $ModelInitName::SequenceProcessString(a) => { simu.process_event($crate::components::sequence::SequenceProcess::<Option<String>, (), Option<String>>::update_state, notif_meta, a.clone()) },
                    a => {
                        <Self as CustomInit>::initialise(a, simu)
                    }
                }
            }
        }

        // pub enum $ModelScheduledEventName {
        //     VectorStockF64LowCapacityChange($crate::nexosim::Address<$crate::components::vector::VectorStock<f64>>, $crate::nexosim::MonotonicTime, f64),
        //     VectorStockF64MaxCapacityChange($crate::nexosim::Address<$crate::components::vector::VectorStock<f64>>, $crate::nexosim::MonotonicTime, f64),
        //     VectorProcessF64ProcessQuantityChange($crate::nexosim::Address<$crate::components::vector::VectorProcess<f64, f64, f64>>, $crate::nexosim::MonotonicTime, $crate::common::Distribution),
        //     VectorProcessF64ProcessTimeChange($crate::nexosim::Address<$crate::components::vector::VectorProcess<f64, f64, f64>>, $crate::nexosim::MonotonicTime, $crate::common::Distribution),
        // }

        #[derive(Display)]
        pub enum $ScheduledEventConfig {
            SetLowCapacity(f64),
            SetMaxCapacity(f64),
            SetProcessQuantity(DistributionConfig),
            SetProcessTime(DistributionConfig),
        }

        // pub enum $ScheduledEvent {
        //     SetLowCapacityVectorStockF64($ScheduledEventConfig, Address<$crate::components::vector::VectorStock<f64>>),
        //     SetMaxCapacityVectorStockF64($ScheduledEventConfig, Address<$crate::components::vector::VectorStock<f64>>),
        // }

        impl $ScheduledEventConfig {
            pub fn schedule_event(self, time: $crate::nexosim::MonotonicTime, scheduler: &mut $crate::nexosim::Scheduler, addr: $ComponentsAddressName, df: &mut DistributionFactory) -> Result<(), Box<dyn ::std::error::Error>> {
                match (self, addr) {
                    ($ScheduledEventConfig::SetLowCapacity(low_capacity), $ComponentsAddressName::VectorStockF64(addr)) => {
                        scheduler.schedule_event(time, $crate::components::vector::VectorStock::<f64>::with_low_capacity_inplace, low_capacity, addr.clone());
                        scheduler.schedule_event(time, $crate::components::vector::VectorStock::<f64>::emit_change, NotificationMetadata {
                            time,
                            element_from: "Scheduler".into(),
                            message: "Low capacity change".into(),
                        }, addr.clone());
                        Ok(())
                    },
                    ($ScheduledEventConfig::SetProcessQuantity(distr), $ComponentsAddressName::VectorProcessF64(addr)) => {
                        let distr_instance = df.create(distr)?;
                        scheduler.schedule_event(time, $crate::components::vector::VectorProcess::<f64, f64, f64>::with_process_quantity_distr_inplace, distr_instance, addr.clone());
                        scheduler.schedule_event(time, $crate::components::vector::VectorProcess::<f64, f64, f64>::update_state, NotificationMetadata {
                            time,
                            element_from: "Scheduler".into(),
                            message: "Process quantity change".into(),
                        }, addr.clone());
                        Ok(())
                    }
                    // $ScheduledEventConfig::VectorStockF64LowCapacityChange(addr, time, low_capacity) => {
                    //     scheduler.schedule_event(time, $crate::components::vector::VectorStock::<f64>::with_low_capacity, low_capacity, addr);
                    //     scheduler.schedule_event(time, $crate::components::vector::VectorStock::<f64>::update_state, $crate::core::NotificationMetadata {
                    //         time,
                    //         element_from: "Scheduler".into(),
                    //         message: "Low capacity change".into(),
                    //     }, addr);
                    //     Ok(())
                    // },
                    (a, b) => {
                        Err(format!("schedule_event not implemented for types ({}, {})", a, b).into())
                    }
                    // $ModelScheduledEventName::VectorStockF64MaxCapacityChange(addr, time, max_capacity) => {
                    //     scheduler.schedule_event($crate::components::vector::VectorStock::<f64>::with_max_capacity, addr, component, max_capacity);
                    // },
                    // $ModelScheduledEventName::VectorProcessF64ProcessQuantityChange(addr, distribution) => {
                    //     scheduler.schedule_event($crate::components::vector::VectorProcess::<f64, f64, f64>::process_quantity_change, addr, distribution);
                    // },
                    // $ModelScheduledEventName::VectorProcessF64ProcessTimeChange(addr, distribution) => {
                    //     scheduler.schedule_event($crate::components::vector::VectorProcess::<f64, f64, f64>::process_time_change, addr, distribution);
                    // }
                }
            }
        }

        #[macro_export]
        macro_rules! connect_components {
            (&mut $a:ident, &mut $b:ident) => {
                $ComponentsName::connect_components(&mut $a, &mut $b, None)
            };
            (&mut $a:ident, &mut $b:ident, $n:expr) => {
                $ComponentsName::connect_components(&mut $a, &mut $b, Some($n))
            };
        }

        #[macro_export]
        macro_rules! connect_logger {
            (&mut $a:ident, &mut $b:ident) => {
                $LoggersName::connect_logger(&mut $a, &mut $b, None)
            };
            (&mut $a:ident, &mut $b:ident, $n:expr) => {
                $LoggersName::connect_logger(&mut $a, &mut $b, Some($n))
            };
        }

        macro_rules! register_component {
            ($sim_init:ident, &mut $init_configs:ident, $component:ident) => {
                $ComponentsName::register_component($sim_init, &mut $init_configs, $component)
            };
        }

        macro_rules! create_scheduled_event {
            // ($scheduler:ident, $event:ident, $component:ident) => {
            //     scheduler.schedule_event($ModelScheduledEventName::schedule_event($sim, $event, $component))
            // };
            (&mut $scheduler:ident, $time:ident, $event_config:ident, $component_addr:ident, &mut $df:ident) => {
                $ScheduledEventConfig::schedule_event($event_config, $time, &mut $scheduler, $component_addr, &mut $df);
            }
        }
    }
}
