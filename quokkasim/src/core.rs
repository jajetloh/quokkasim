use std::{error::Error, fmt::Debug, fs::File};
use csv::WriterBuilder;
use crate::prelude::*;
use serde::{ser::SerializeStruct, Serialize};
use tai_time::MonotonicTime;

impl ResourceAdd<f64> for f64 {
    fn add(&mut self, arg: f64) {
        *self += arg;
    }
}

impl ResourceRemove<f64, f64> for f64 {
    fn remove(&mut self, arg: f64) -> f64 {
        *self -= arg;
        arg
    }
}

impl ResourceTotal<f64> for f64 {
    fn total(&self) -> f64 {
        *self
    }
}

impl ResourceMultiply<f64> for f64 {
    fn multiply(&mut self, factor: f64) {
        *self *= factor;
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

impl ResourceAdd<Vector3> for Vector3 {
    fn add(&mut self, arg: Vector3) {
        self.values[0] += arg.values[0];
        self.values[1] += arg.values[1];
        self.values[2] += arg.values[2];
    }
}

impl ResourceRemove<f64, Vector3> for Vector3 {
    fn remove(&mut self, arg: f64) -> Vector3 {
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
}

impl ResourceTotal<f64> for Vector3 {
    fn total(&self) -> f64 {
        self.values.iter().sum()
    }
}

impl ResourceMultiply<f64> for Vector3 {
    fn multiply(&mut self, factor: f64) {
        self.values[0] *= factor;
        self.values[1] *= factor;
        self.values[2] *= factor;
    }
}

impl From<[f64; 3]> for Vector3 {
    fn from(val: [f64; 3]) -> Self {
        Vector3 { values: val }
    }
}

impl Default for Vector3 {
    fn default() -> Self {
        Vector3 { values: [0.0, 0.0, 0.0] }
    }
}

pub trait ResourceAdd<T> {
    fn add(&mut self, arg: T);
}

pub trait ResourceRemove<T, U> {
    fn remove(&mut self, arg: T) -> U;
}

pub trait ResourceTotal<C> {
    fn total(&self) -> C;
}

pub trait ResourceMultiply<T> {
    fn multiply(&mut self, factor: T);
}

pub trait StateEq {
    fn is_same_state(&self, other: &Self) -> bool;
}

pub trait Stock<
    ContainerType: Clone + 'static,
    ReceiveType: Clone + Send + 'static,
    SendParameterType: Clone + Send + 'static,
    SendType: Clone + Send + 'static,
> where Self: Model {
    type StockState: StateEq + Clone + Debug;
    type LogDetailsType: Clone;

    fn pre_add(&mut self, payload: &mut (ReceiveType, EventId), cx: &mut Context<Self>) -> impl Future<Output = ()> + {
        async move {
            self.set_previous_state();
        }
    }

    fn add_impl(&mut self, payload: &mut (ReceiveType, EventId), cx: &mut Context<Self>) -> impl Future<Output = ()> + ;

    fn post_add(&mut self, payload: &mut (ReceiveType, EventId), cx: &mut Context<Self>) -> impl Future<Output = ()> + {
        async move {
            // let mut source_event_id = payload.1.clone();
            // let current_state = self.get_state();
            // let previous_state: &Option<Self::StockState> = self.get_previous_state();
            // match previous_state {
            //     None => {},
            //     Some(ps) => {
            //         let ps = ps.clone();
            //         source_event_id = self.log(cx.time(), source_event_id.clone(), "StockAdd").await;
            //         if !ps.is_same_state(&current_state) {
            //             source_event_id = self.log(cx.time(), source_event_id.clone(), "StateChange").await;
            //             // State change emission is scheduled 1ns in future to avoid infinite state change loops with processes
            //             cx.schedule_event(
            //                 cx.time() + ::std::time::Duration::from_nanos(1),
            //                 Self::emit_change,
            //                 source_event_id,
            //             ).unwrap();
            //         } else {
            //         }
            //     }
            // }
        }
    }

    fn emit_change(&mut self, payload: EventId, cx: &mut nexosim::model::Context<Self>) -> impl Future<Output = ()> + Send;

    fn add(&mut self, mut payload: (ReceiveType, EventId), cx: &mut Context<Self>) -> impl Future<Output = ()> + where ReceiveType: 'static {
        async move {
            self.pre_add(&mut payload, cx).await;
            self.add_impl(&mut payload, cx).await;
            self.post_add(&mut payload, cx).await;
        }
    }

    fn pre_remove(&mut self, payload: &mut (SendParameterType, EventId), cx: &mut Context<Self>) -> impl Future<Output = ()> + {
        async move {
            self.set_previous_state();
        }
    }

    fn remove_impl(&mut self, payload: &mut (SendParameterType, EventId), cx: &mut Context<Self>) -> impl Future<Output = SendType>;

    fn post_remove(&mut self, payload: &mut (SendParameterType, EventId), cx: &mut Context<Self>) -> impl Future<Output = ()> + {
        async move {
            // let mut event_id: EventId = payload.1.clone();
            // let current_state = self.get_state();
            // let previous_state: &Option<Self::StockState> = self.get_previous_state();
            // match previous_state {
            //     None => {},
            //     Some(ps) => {
            //         let ps = ps.clone();
            //         if !ps.is_same_state(&current_state) {
            //             event_id = self.log(cx.time(), event_id, ps).await;
            //             // State change emission is scheduled 1ns in future to avoid infinite state change loops with processes
            //             cx.schedule_event(
            //                 cx.time() + ::std::time::Duration::from_nanos(1),
            //                 Self::emit_change,
            //                 event_id,
            //             ).unwrap();
            //         } else {
            //         }
            //     }
            // }
        }
    }

    fn remove(&mut self, mut payload: (SendParameterType, EventId), cx: &mut Context<Self>) -> impl Future<Output = SendType> + where SendParameterType: 'static {
        async move {
            self.pre_remove(&mut payload, cx).await;
            let  result = self.remove_impl(&mut payload, cx).await;
            self.post_remove(&mut payload, cx).await;
            result
        }
    }

    fn get_state_async(&mut self) -> impl Future<Output = Self::StockState> + {
        async move {
            self.get_state()
        }
    }
    fn get_state(&mut self) -> Self::StockState;
    fn get_resource(&self) -> &ContainerType;
    fn get_previous_state(&mut self) -> &Option<Self::StockState>;
    fn set_previous_state(&mut self);
    fn log(&mut self, now: MonotonicTime, source_event: EventId, details: Self::LogDetailsType) -> impl Future<Output = EventId> + Send;
}

pub struct BasicEnvironment {
    pub element_name: String,
    pub element_code: String,
    pub state: BasicEnvironmentState,
    pub next_event_index: u64,
    pub log_emitter: Output<BasicEnvironmentLog>,
    pub emit_change: Output<EventId>,
}

impl Model for BasicEnvironment {
    fn init(mut self, cx: &mut Context<Self>) -> impl Future<Output = InitializedModel<Self>> + Send {
        async move {
            self.log(cx.time(), EventId::from_init(), self.state.clone()).await;
            self.into()
        }
    }
}

impl Default for BasicEnvironment {
    fn default() -> Self {
        Self::new()
    }
}

impl BasicEnvironment {
    pub fn new() -> Self {
        BasicEnvironment {
            element_name: String::new(),
            element_code: String::new(),
            state: BasicEnvironmentState::Normal,
            next_event_index: 0,
            log_emitter: Output::default(),
            emit_change: Output::default(),
        }
    }

    pub fn with_name(mut self, name: String) -> Self {
        self.element_name = name;
        self
    }

    pub fn with_code(mut self, code: String) -> Self {
        self.element_code = code;
        self
    }

    pub fn with_state(mut self, state: BasicEnvironmentState) -> Self {
        self.state = state;
        self
    }

    pub fn set_state(&mut self, payload: (BasicEnvironmentState, EventId), cx: &mut Context<Self>) -> impl Future<Output = ()> {
        async move {
            let (state, mut event_id) = payload;
            if self.state != state {
                self.state = state;
                event_id = self.log(cx.time(), event_id, self.state.clone()).await;
                self.emit_change.send(event_id).await;
            }
        }
    }

    pub fn get_state_async(&mut self) -> impl Future<Output = BasicEnvironmentState> + {
        async move {
            self.state.clone()
        }
    }

    fn log(&mut self, now: MonotonicTime, source_event_id: EventId, event: BasicEnvironmentState) -> impl Future<Output = EventId> + Send {
        async move {
            let new_event_id = EventId(format!("{}_{:06}", self.element_code, self.next_event_index));
            let log = BasicEnvironmentLog {
                time: now.to_string(),
                event_id: new_event_id.clone(),
                source_event_id,
                element_name: self.element_name.clone(),
                element_type: "BasicEnvironment".to_string(),
                event,
            };
            self.log_emitter.send(log).await;
            self.next_event_index += 1;

            new_event_id
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum BasicEnvironmentState {
    Normal,
    Stopped,
}

#[derive(Clone)]
pub struct BasicEnvironmentLog {
    pub time: String,
    pub event_id: EventId,
    pub source_event_id: EventId,
    pub element_name: String,
    pub element_type: String,
    pub event: BasicEnvironmentState,
}

impl Serialize for BasicEnvironmentLog {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("BasicEnvironmentLog", 5)?;
        state.serialize_field("time", &self.time)?;
        state.serialize_field("event_id", &self.event_id)?;
        state.serialize_field("source_event_id", &self.source_event_id)?;
        state.serialize_field("element_name", &self.element_name)?;
        state.serialize_field("element_type", &self.element_type)?;
        state.serialize_field("event", match &self.event {
            BasicEnvironmentState::Normal => "Normal",
            BasicEnvironmentState::Stopped => "Stopped",
        })?;
        state.end()
    }
}

pub struct BasicEnvironmentLogger {
    pub name: String,
    pub buffer: EventQueue<BasicEnvironmentLog>,
}

impl Logger for BasicEnvironmentLogger {
    type RecordType = BasicEnvironmentLog;

    fn new(name: String) -> Self {
        BasicEnvironmentLogger {
            name,
            buffer: EventQueue::new(),
        }
    }

    fn get_name(&self) -> &String {
        &self.name
    }

    fn get_buffer(self) -> EventQueue<Self::RecordType> {
        self.buffer
    }

    fn write_csv(self, dir: String) -> Result<(), Box<dyn Error>> {
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
}

pub trait Process {
    type LogDetailsType;

    fn pre_update_state(&mut self, source_event_id: &mut EventId, cx: &mut Context<Self>) -> impl Future<Output = ()> + Send where Self: Model;

    fn update_state_impl(&mut self, source_event_id: &mut EventId, cx: &mut Context<Self>) -> impl Future<Output = ()> + Send where Self: Model;

    fn update_state(&mut self, mut source_event_id: EventId, cx: &mut Context<Self>) -> impl Future<Output = ()> + Send where Self: Model {
        async move {
            self.pre_update_state(&mut source_event_id, cx).await;
            self.update_state_impl(&mut source_event_id, cx).await;
            self.post_update_state(&mut source_event_id, cx).await;
        }
    }

    fn post_update_state(&mut self, source_event_id: &mut EventId, cx: &mut Context<Self>) -> impl Future<Output = ()> + Send where Self: Model;

    fn log(&mut self, now: MonotonicTime, source_event: EventId, details: Self::LogDetailsType) -> impl Future<Output = EventId>;

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
        }

        $(#[$logger_enum_meta:meta])*
        pub enum $ComponentLogger:ident {
            $(
                $(#[$logger_var_meta:meta])*
                $U:ident $( ( $UT:ty ) )?
            ),* $(,)?
        }

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
            VectorProcessF64($crate::components::vector::VectorProcess<f64, f64, f64, f64>, $crate::nexosim::Mailbox<$crate::components::vector::VectorProcess<f64, f64, f64, f64>>),
            VectorSourceF64($crate::components::vector::VectorSource<f64, f64>, $crate::nexosim::Mailbox<$crate::components::vector::VectorSource<f64, f64>>),
            VectorSinkF64($crate::components::vector::VectorSink<f64, f64, f64>, $crate::nexosim::Mailbox<$crate::components::vector::VectorSink<f64, f64, f64>>),
            VectorCombiner1F64($crate::components::vector::VectorCombiner<f64, f64, f64, [f64; 1], 1>, $crate::nexosim::Mailbox<$crate::components::vector::VectorCombiner<f64, f64, [f64; 1], f64, 1>>),
            VectorCombiner2F64($crate::components::vector::VectorCombiner<f64, f64, f64, [f64; 2], 2>, $crate::nexosim::Mailbox<$crate::components::vector::VectorCombiner<f64, f64, [f64; 2], f64, 2>>),
            VectorCombiner3F64($crate::components::vector::VectorCombiner<f64, f64, f64, [f64; 3], 3>, $crate::nexosim::Mailbox<$crate::components::vector::VectorCombiner<f64, f64, [f64; 3], f64, 3>>),
            VectorCombiner4F64($crate::components::vector::VectorCombiner<f64, f64, f64, [f64; 4], 4>, $crate::nexosim::Mailbox<$crate::components::vector::VectorCombiner<f64, f64, [f64; 4], f64, 4>>),
            VectorCombiner5F64($crate::components::vector::VectorCombiner<f64, f64, f64, [f64; 5], 5>, $crate::nexosim::Mailbox<$crate::components::vector::VectorCombiner<f64, f64, [f64; 5], f64, 5>>),
            VectorSplitter1F64($crate::components::vector::VectorSplitter<f64, f64, f64, f64, 1>, $crate::nexosim::Mailbox<$crate::components::vector::VectorSplitter<f64, f64, f64, f64, 1>>),
            VectorSplitter2F64($crate::components::vector::VectorSplitter<f64, f64, f64, f64, 2>, $crate::nexosim::Mailbox<$crate::components::vector::VectorSplitter<f64, f64, f64, f64, 2>>),
            VectorSplitter3F64($crate::components::vector::VectorSplitter<f64, f64, f64, f64, 3>, $crate::nexosim::Mailbox<$crate::components::vector::VectorSplitter<f64, f64, f64, f64, 3>>),
            VectorSplitter4F64($crate::components::vector::VectorSplitter<f64, f64, f64, f64, 4>, $crate::nexosim::Mailbox<$crate::components::vector::VectorSplitter<f64, f64, f64, f64, 4>>),
            VectorSplitter5F64($crate::components::vector::VectorSplitter<f64, f64, f64, f64, 5>, $crate::nexosim::Mailbox<$crate::components::vector::VectorSplitter<f64, f64, f64, f64, 5>>),
            VectorStockVector3($crate::components::vector::VectorStock<Vector3>, $crate::nexosim::Mailbox<$crate::components::vector::VectorStock<Vector3>>),
            VectorProcessVector3($crate::components::vector::VectorProcess<f64, Vector3, Vector3, Vector3>, $crate::nexosim::Mailbox<$crate::components::vector::VectorProcess<f64, Vector3, Vector3, Vector3>>),
            VectorSourceVector3($crate::components::vector::VectorSource<Vector3, Vector3>, $crate::nexosim::Mailbox<$crate::components::vector::VectorSource<Vector3, Vector3>>),
            VectorSinkVector3($crate::components::vector::VectorSink<f64, Vector3, Vector3>, $crate::nexosim::Mailbox<$crate::components::vector::VectorSink<f64, Vector3, Vector3>>),
            VectorCombiner1Vector3($crate::components::vector::VectorCombiner<f64, Vector3, [Vector3; 1], Vector3, 1>, $crate::nexosim::Mailbox<$crate::components::vector::VectorCombiner<f64, Vector3, [Vector3; 1], Vector3, 1>>),
            VectorCombiner2Vector3($crate::components::vector::VectorCombiner<f64, Vector3, [Vector3; 2], Vector3, 2>, $crate::nexosim::Mailbox<$crate::components::vector::VectorCombiner<f64, Vector3, [Vector3; 2], Vector3, 2>>),
            VectorCombiner3Vector3($crate::components::vector::VectorCombiner<f64, Vector3, [Vector3; 3], Vector3, 3>, $crate::nexosim::Mailbox<$crate::components::vector::VectorCombiner<f64, Vector3, [Vector3; 3], Vector3, 3>>),
            VectorCombiner4Vector3($crate::components::vector::VectorCombiner<f64, Vector3, [Vector3; 4], Vector3, 4>, $crate::nexosim::Mailbox<$crate::components::vector::VectorCombiner<f64, Vector3, [Vector3; 4], Vector3, 4>>),
            VectorCombiner5Vector3($crate::components::vector::VectorCombiner<f64, Vector3, [Vector3; 5], Vector3, 5>, $crate::nexosim::Mailbox<$crate::components::vector::VectorCombiner<f64, Vector3, [Vector3; 5], Vector3, 5>>),
            VectorSplitter1Vector3($crate::components::vector::VectorSplitter<f64, Vector3, Vector3, Vector3, 1>, $crate::nexosim::Mailbox<$crate::components::vector::VectorSplitter<f64, Vector3, Vector3, Vector3, 1>>),
            VectorSplitter2Vector3($crate::components::vector::VectorSplitter<f64, Vector3, Vector3, Vector3, 2>, $crate::nexosim::Mailbox<$crate::components::vector::VectorSplitter<f64, Vector3, Vector3, Vector3, 2>>),
            VectorSplitter3Vector3($crate::components::vector::VectorSplitter<f64, Vector3, Vector3, Vector3, 3>, $crate::nexosim::Mailbox<$crate::components::vector::VectorSplitter<f64, Vector3, Vector3, Vector3, 3>>),
            VectorSplitter4Vector3($crate::components::vector::VectorSplitter<f64, Vector3, Vector3, Vector3, 4>, $crate::nexosim::Mailbox<$crate::components::vector::VectorSplitter<f64, Vector3, Vector3, Vector3, 4>>),
            VectorSplitter5Vector3($crate::components::vector::VectorSplitter<f64, Vector3, Vector3, Vector3, 5>, $crate::nexosim::Mailbox<$crate::components::vector::VectorSplitter<f64, Vector3, Vector3, Vector3, 5>>),
            DiscreteStockString($crate::components::discrete::DiscreteStock<String>, $crate::nexosim::Mailbox<$crate::components::discrete::DiscreteStock<String>>),
            DiscreteProcessString($crate::components::discrete::DiscreteProcess<(), Option<String>, String, String>, $crate::nexosim::Mailbox<$crate::components::discrete::DiscreteProcess<(), Option<String>, String, String>>),
            DiscreteParallelProcessString($crate::components::discrete::DiscreteParallelProcess<(), Option<String>, String, String>, $crate::nexosim::Mailbox<$crate::components::discrete::DiscreteParallelProcess<(), Option<String>, String, String>>),
            DiscreteSourceString($crate::components::discrete::DiscreteSource<String, String, StringItemFactory>, $crate::nexosim::Mailbox<$crate::components::discrete::DiscreteSource<String, String, StringItemFactory>>),
            DiscreteSinkString($crate::components::discrete::DiscreteSink<(), Option<String>, String>, $crate::nexosim::Mailbox<$crate::components::discrete::DiscreteSink<(), Option<String>, String>>),
            BasicEnvironment(BasicEnvironment, $crate::nexosim::Mailbox<BasicEnvironment>),
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
                    $ComponentModel::BasicEnvironment(a, mb) => {
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
                    $ComponentModel::BasicEnvironment(_, mb) => $ComponentModelAddress::BasicEnvironment(mb.address()),
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
            VectorProcessF64($crate::nexosim::Address<$crate::components::vector::VectorProcess<f64, f64, f64, f64>>),
            VectorSourceF64($crate::nexosim::Address<$crate::components::vector::VectorSource<f64, f64>>),
            VectorSinkF64($crate::nexosim::Address<$crate::components::vector::VectorSink<f64, f64, f64>>),
            VectorCombiner1F64($crate::nexosim::Address<$crate::components::vector::VectorCombiner<f64, f64, [f64; 1], f64, 1>>),
            VectorCombiner2F64($crate::nexosim::Address<$crate::components::vector::VectorCombiner<f64, f64, [f64; 2], f64, 2>>),
            VectorCombiner3F64($crate::nexosim::Address<$crate::components::vector::VectorCombiner<f64, f64, [f64; 3], f64, 3>>),
            VectorCombiner4F64($crate::nexosim::Address<$crate::components::vector::VectorCombiner<f64, f64, [f64; 4], f64, 4>>),
            VectorCombiner5F64($crate::nexosim::Address<$crate::components::vector::VectorCombiner<f64, f64, [f64; 5], f64, 5>>),
            VectorSplitter1F64($crate::nexosim::Address<$crate::components::vector::VectorSplitter<f64, f64, f64, f64, 1>>),
            VectorSplitter2F64($crate::nexosim::Address<$crate::components::vector::VectorSplitter<f64, f64, f64, f64, 2>>),
            VectorSplitter3F64($crate::nexosim::Address<$crate::components::vector::VectorSplitter<f64, f64, f64, f64, 3>>),
            VectorSplitter4F64($crate::nexosim::Address<$crate::components::vector::VectorSplitter<f64, f64, f64, f64, 4>>),
            VectorSplitter5F64($crate::nexosim::Address<$crate::components::vector::VectorSplitter<f64, f64, f64, f64, 5>>),
            VectorStockVector3($crate::nexosim::Address<$crate::components::vector::VectorStock<Vector3>>),
            VectorProcessVector3($crate::nexosim::Address<$crate::components::vector::VectorProcess<f64, Vector3, Vector3, Vector3>>),
            VectorSourceVector3($crate::nexosim::Address<$crate::components::vector::VectorSource<Vector3, Vector3>>),
            VectorSinkVector3($crate::nexosim::Address<$crate::components::vector::VectorSink<f64, Vector3, Vector3>>),
            VectorCombiner1Vector3($crate::nexosim::Address<$crate::components::vector::VectorCombiner<f64, Vector3, [Vector3; 1], Vector3, 1>>),
            VectorCombiner2Vector3($crate::nexosim::Address<$crate::components::vector::VectorCombiner<f64, Vector3, [Vector3; 2], Vector3, 2>>),
            VectorCombiner3Vector3($crate::nexosim::Address<$crate::components::vector::VectorCombiner<f64, Vector3, [Vector3; 3], Vector3, 3>>),
            VectorCombiner4Vector3($crate::nexosim::Address<$crate::components::vector::VectorCombiner<f64, Vector3, [Vector3; 4], Vector3, 4>>),
            VectorCombiner5Vector3($crate::nexosim::Address<$crate::components::vector::VectorCombiner<f64, Vector3, [Vector3; 5], Vector3, 5>>),
            VectorSplitter1Vector3($crate::nexosim::Address<$crate::components::vector::VectorSplitter<f64, Vector3, Vector3, Vector3, 1>>),
            VectorSplitter2Vector3($crate::nexosim::Address<$crate::components::vector::VectorSplitter<f64, Vector3, Vector3, Vector3, 2>>),
            VectorSplitter3Vector3($crate::nexosim::Address<$crate::components::vector::VectorSplitter<f64, Vector3, Vector3, Vector3, 3>>),
            VectorSplitter4Vector3($crate::nexosim::Address<$crate::components::vector::VectorSplitter<f64, Vector3, Vector3, Vector3, 4>>),
            VectorSplitter5Vector3($crate::nexosim::Address<$crate::components::vector::VectorSplitter<f64, Vector3, Vector3, Vector3, 5>>),
            DiscreteStockString($crate::nexosim::Address<$crate::components::discrete::DiscreteStock<String>>),
            DiscreteProcessString($crate::nexosim::Address<$crate::components::discrete::DiscreteProcess<(), Option<String>, String, String>>),
            DiscreteParallelProcessString($crate::nexosim::Address<$crate::components::discrete::DiscreteParallelProcess<(), Option<String>, String, String>>),
            DiscreteSourceString($crate::nexosim::Address<$crate::components::discrete::DiscreteSource<String, String, StringItemFactory>>),
            DiscreteSinkString($crate::nexosim::Address<$crate::components::discrete::DiscreteSink<(), Option<String>, String>>),
            BasicEnvironment($crate::nexosim::Address<BasicEnvironment>),
            $(
                $R $( ($crate::nexosim::Address<$RT>) )?
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
            DiscreteProcessLoggerString($crate::components::discrete::DiscreteProcessLogger<String>),

            BasicEnvironmentLogger(BasicEnvironmentLogger),
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
                    ($ComponentLogger::BasicEnvironmentLogger(a), $ComponentModel::BasicEnvironment(b, bd), _) => {
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

                    $ComponentLogger::BasicEnvironmentLogger(a) => { a.write_csv(dir.to_string()) },

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

        #[derive(Display)]
        pub enum $ScheduledEventConfig {
            SetLowCapacity(f64),
            SetMaxCapacity(f64),
            SetProcessQuantity(DistributionConfig),
            SetProcessTime(DistributionConfig),
            SetEnvironmentState(BasicEnvironmentState),
        }

        impl $ScheduledEventConfig {
            pub fn schedule_event(&self, time: &$crate::nexosim::MonotonicTime, scheduler: &mut $crate::nexosim::Scheduler, addr: &$ComponentModelAddress, df: &mut DistributionFactory) -> Result<(), Box<dyn ::std::error::Error>> {
                let time = time.clone();
                let source_event_id = $crate::prelude::EventId::from_scheduler();
                match (self, addr) {
                    ($ScheduledEventConfig::SetLowCapacity(low_capacity), $ComponentModelAddress::VectorStockF64(addr)) => {
                        scheduler.schedule_event(time, $crate::components::vector::VectorStock::<f64>::with_low_capacity_inplace, low_capacity.clone(), addr.clone())?;
                        scheduler.schedule_event(time, $crate::components::vector::VectorStock::<f64>::emit_change, source_event_id, addr.clone())?;
                        Ok(())
                    },
                    ($ScheduledEventConfig::SetProcessQuantity(distr), $ComponentModelAddress::VectorProcessF64(addr)) => {
                        let distr_instance = df.create(distr.clone())?;
                        scheduler.schedule_event(time, $crate::components::vector::VectorProcess::<f64, f64, f64, f64>::with_process_quantity_distr_inplace, distr_instance, addr.clone())?;
                        scheduler.schedule_event(time, $crate::components::vector::VectorProcess::<f64, f64, f64, f64>::update_state, source_event_id, addr.clone())?;
                        Ok(())
                    },
                    ($ScheduledEventConfig::SetEnvironmentState(env_state), $ComponentModelAddress::BasicEnvironment(addr)) => {
                        scheduler.schedule_event(time, BasicEnvironment::set_state, (env_state.clone(), source_event_id), addr.clone())?;
                        Ok(())
                    },
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
            (&mut $scheduler:ident, &$time:ident, &$event_config:ident, &$component_addr:ident, &mut $df:ident) => {
                $ScheduledEventConfig::schedule_event(&$event_config, &$time, &mut $scheduler, &$component_addr, &mut $df)
            }
        }
    }
}
