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

impl ResourceRemoveAll<f64> for f64 {
    fn remove_all(&mut self) -> f64 {
        let removed = *self;
        *self = 0.0;
        removed
    }
}

pub trait ResourceContainer<T, U> {
    fn set_resource(&mut self, resource: Option<T>);
    fn get_resource(&self) -> Option<T>;
    fn take_resource(&mut self) -> Option<T>;
    fn get_capacity(&self) -> U;
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

impl ResourceRemoveAll<Vector3> for Vector3 {
    fn remove_all(&mut self) -> Vector3 {
        let removed = self.clone();
        *self = Vector3::default();
        removed
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

pub trait ResourceRemoveAll<U> {
    fn remove_all(&mut self) -> U;
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
        async move {}
    }

    fn emit_change(&mut self, payload: (Self::StockState, EventId), cx: &mut nexosim::model::Context<Self>) -> impl Future<Output = ()> + Send;

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
        async move {}
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

#[derive(WithMethods)]
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
        BasicEnvironment {
            element_name: String::new(),
            element_code: String::new(),
            state: BasicEnvironmentState::Normal,
            next_event_index: 0,
            log_emitter: Output::default(),
            emit_change: Output::default(),
        }
    }
}

impl BasicEnvironment {
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

    fn new(name: &str) -> Self {
        BasicEnvironmentLogger {
            name: name.into(),
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
    fn new(name: &str) -> Self;
}

pub trait CustomComponentConnection {
    fn connect_components(a: &mut Self, b: &mut Self, n: Option<usize>) -> Result<(), Box<dyn ::std::error::Error>>;
}

pub trait CustomLoggerConnection {
    type ComponentType;
    fn connect_logger(a: &mut Self, b: &mut Self::ComponentType, n: Option<usize>) -> Result<(), Box<dyn ::std::error::Error>>;
}

#[macro_export]
macro_rules! register_component_arms {
    ($component:ident, $sim_init:ident, $($variant:ident),* $(,)?) => {
        match $component {
            $(
                ComponentModel::$variant(a, mb) => {
                    let name = a.element_name.clone();
                    let addr = mb.address();
                    $sim_init = $sim_init.add_model(a, mb, name);
                }
            ),*
            _ => {
                panic!("Component type not implemented for registration");
            }

        }
    };
}

#[macro_export]
macro_rules! get_address_arms {
    ($component:ident, $($variant:ident),* $(,)?) => {
        match $component {
            $(
                ComponentModel::$variant(_, mb) => ComponentModelAddress::$variant(mb.address()),
            )*
            _ => {
                panic!("Component type not implemented for address retrieval");
            }
        }
    };
}

#[macro_export]
macro_rules! connect_logger_arms {
    (
        $a:ident, $b:ident, $n:ident, $(
            $logger_variant:ident => [ $($component_variant:ident),* $(,)? ]
        ),* $(,)?
    ) => {
        match ($a, $b, $n) {
            $($(
                (ComponentLogger::$logger_variant(a), ComponentModel::$component_variant(b, _), _) => {
                    b.log_emitter.connect_sink(&a.buffer);
                    Ok(())
                },
            )*)*
            ($a, $b, $n) => <ComponentLogger as CustomLoggerConnection>::connect_logger($a, $b, $n)
        }
    };
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
        use ::quokkasim::components::vector_container::{F64Container, F64ContainerFactory, Vector3Container, Vector3ContainerFactory};

        $(#[$components_enum_meta])*
        #[derive(Display)]
        pub enum $ComponentModel {
            F64Stock($crate::components::vector::VectorStock<f64>, $crate::nexosim::Mailbox<$crate::components::vector::VectorStock<f64>>),
            F64Process($crate::components::vector::VectorProcess<f64, f64, f64, f64>, $crate::nexosim::Mailbox<$crate::components::vector::VectorProcess<f64, f64, f64, f64>>),
            F64Source($crate::components::vector::VectorSource<f64, f64>, $crate::nexosim::Mailbox<$crate::components::vector::VectorSource<f64, f64>>),
            F64Sink($crate::components::vector::VectorSink<f64, f64, f64>, $crate::nexosim::Mailbox<$crate::components::vector::VectorSink<f64, f64, f64>>),
            F64Combiner1($crate::components::vector::VectorCombiner<f64, f64, [f64; 1], f64, 1>, $crate::nexosim::Mailbox<$crate::components::vector::VectorCombiner<f64, f64, [f64; 1], f64, 1>>),
            F64Combiner2($crate::components::vector::VectorCombiner<f64, f64, [f64; 2], f64, 2>, $crate::nexosim::Mailbox<$crate::components::vector::VectorCombiner<f64, f64, [f64; 2], f64, 2>>),
            F64Combiner3($crate::components::vector::VectorCombiner<f64, f64, [f64; 3], f64, 3>, $crate::nexosim::Mailbox<$crate::components::vector::VectorCombiner<f64, f64, [f64; 3], f64, 3>>),
            F64Combiner4($crate::components::vector::VectorCombiner<f64, f64, [f64; 4], f64, 4>, $crate::nexosim::Mailbox<$crate::components::vector::VectorCombiner<f64, f64, [f64; 4], f64, 4>>),
            F64Combiner5($crate::components::vector::VectorCombiner<f64, f64, [f64; 5], f64, 5>, $crate::nexosim::Mailbox<$crate::components::vector::VectorCombiner<f64, f64, [f64; 5], f64, 5>>),
            F64Splitter1($crate::components::vector::VectorSplitter<f64, f64, f64, f64, 1>, $crate::nexosim::Mailbox<$crate::components::vector::VectorSplitter<f64, f64, f64, f64, 1>>),
            F64Splitter2($crate::components::vector::VectorSplitter<f64, f64, f64, f64, 2>, $crate::nexosim::Mailbox<$crate::components::vector::VectorSplitter<f64, f64, f64, f64, 2>>),
            F64Splitter3($crate::components::vector::VectorSplitter<f64, f64, f64, f64, 3>, $crate::nexosim::Mailbox<$crate::components::vector::VectorSplitter<f64, f64, f64, f64, 3>>),
            F64Splitter4($crate::components::vector::VectorSplitter<f64, f64, f64, f64, 4>, $crate::nexosim::Mailbox<$crate::components::vector::VectorSplitter<f64, f64, f64, f64, 4>>),
            F64Splitter5($crate::components::vector::VectorSplitter<f64, f64, f64, f64, 5>, $crate::nexosim::Mailbox<$crate::components::vector::VectorSplitter<f64, f64, f64, f64, 5>>),
            
            Vector3Stock($crate::components::vector::VectorStock<Vector3>, $crate::nexosim::Mailbox<$crate::components::vector::VectorStock<Vector3>>),
            Vector3Process($crate::components::vector::VectorProcess<f64, Vector3, Vector3, Vector3>, $crate::nexosim::Mailbox<$crate::components::vector::VectorProcess<f64, Vector3, Vector3, Vector3>>),
            Vector3Source($crate::components::vector::VectorSource<Vector3, Vector3>, $crate::nexosim::Mailbox<$crate::components::vector::VectorSource<Vector3, Vector3>>),
            Vector3Sink($crate::components::vector::VectorSink<f64, Vector3, Vector3>, $crate::nexosim::Mailbox<$crate::components::vector::VectorSink<f64, Vector3, Vector3>>),
            Vector3Combiner1($crate::components::vector::VectorCombiner<f64, Vector3, [Vector3; 1], Vector3, 1>, $crate::nexosim::Mailbox<$crate::components::vector::VectorCombiner<f64, Vector3, [Vector3; 1], Vector3, 1>>),
            Vector3Combiner2($crate::components::vector::VectorCombiner<f64, Vector3, [Vector3; 2], Vector3, 2>, $crate::nexosim::Mailbox<$crate::components::vector::VectorCombiner<f64, Vector3, [Vector3; 2], Vector3, 2>>),
            Vector3Combiner3($crate::components::vector::VectorCombiner<f64, Vector3, [Vector3; 3], Vector3, 3>, $crate::nexosim::Mailbox<$crate::components::vector::VectorCombiner<f64, Vector3, [Vector3; 3], Vector3, 3>>),
            Vector3Combiner4($crate::components::vector::VectorCombiner<f64, Vector3, [Vector3; 4], Vector3, 4>, $crate::nexosim::Mailbox<$crate::components::vector::VectorCombiner<f64, Vector3, [Vector3; 4], Vector3, 4>>),
            Vector3Combiner5($crate::components::vector::VectorCombiner<f64, Vector3, [Vector3; 5], Vector3, 5>, $crate::nexosim::Mailbox<$crate::components::vector::VectorCombiner<f64, Vector3, [Vector3; 5], Vector3, 5>>),
            Vector3Splitter1($crate::components::vector::VectorSplitter<f64, Vector3, Vector3, Vector3, 1>, $crate::nexosim::Mailbox<$crate::components::vector::VectorSplitter<f64, Vector3, Vector3, Vector3, 1>>),
            Vector3Splitter2($crate::components::vector::VectorSplitter<f64, Vector3, Vector3, Vector3, 2>, $crate::nexosim::Mailbox<$crate::components::vector::VectorSplitter<f64, Vector3, Vector3, Vector3, 2>>),
            Vector3Splitter3($crate::components::vector::VectorSplitter<f64, Vector3, Vector3, Vector3, 3>, $crate::nexosim::Mailbox<$crate::components::vector::VectorSplitter<f64, Vector3, Vector3, Vector3, 3>>),
            Vector3Splitter4($crate::components::vector::VectorSplitter<f64, Vector3, Vector3, Vector3, 4>, $crate::nexosim::Mailbox<$crate::components::vector::VectorSplitter<f64, Vector3, Vector3, Vector3, 4>>),
            Vector3Splitter5($crate::components::vector::VectorSplitter<f64, Vector3, Vector3, Vector3, 5>, $crate::nexosim::Mailbox<$crate::components::vector::VectorSplitter<f64, Vector3, Vector3, Vector3, 5>>),
            
            StringStock($crate::components::discrete::DiscreteStock<String>, $crate::nexosim::Mailbox<$crate::components::discrete::DiscreteStock<String>>),
            StringProcess($crate::components::discrete::DiscreteProcess<(), Option<String>, String, String>, $crate::nexosim::Mailbox<$crate::components::discrete::DiscreteProcess<(), Option<String>, String, String>>),
            StringParallelProcess($crate::components::discrete::DiscreteParallelProcess<(), Option<String>, String, String>, $crate::nexosim::Mailbox<$crate::components::discrete::DiscreteParallelProcess<(), Option<String>, String, String>>),
            StringSource($crate::components::discrete::DiscreteSource<String, String, StringItemFactory>, $crate::nexosim::Mailbox<$crate::components::discrete::DiscreteSource<String, String, StringItemFactory>>),
            StringSink($crate::components::discrete::DiscreteSink<(), Option<String>, String>, $crate::nexosim::Mailbox<$crate::components::discrete::DiscreteSink<(), Option<String>, String>>),
            
            F64ContainerStock($crate::components::discrete::DiscreteStock<F64Container>, $crate::nexosim::Mailbox<$crate::components::discrete::DiscreteStock<F64Container>>),
            F64ContainerProcess($crate::components::discrete::DiscreteProcess<(), Option<F64Container>, F64Container, F64Container>, $crate::nexosim::Mailbox<$crate::components::discrete::DiscreteProcess<(), Option<F64Container>, F64Container, F64Container>>),
            F64ContainerParallelProcess($crate::components::discrete::DiscreteParallelProcess<(), Option<F64Container>, F64Container, F64Container>, $crate::nexosim::Mailbox<$crate::components::discrete::DiscreteParallelProcess<(), Option<F64Container>, F64Container, F64Container>>),
            F64ContainerSource($crate::components::discrete::DiscreteSource<F64Container, F64Container, F64ContainerFactory>, $crate::nexosim::Mailbox<$crate::components::discrete::DiscreteSource<F64Container, F64Container, F64ContainerFactory>>),
            F64ContainerSink($crate::components::discrete::DiscreteSink<(), Option<F64Container>, F64Container>, $crate::nexosim::Mailbox<$crate::components::discrete::DiscreteSink<(), Option<F64Container>, F64Container>>),
            F64ContainerLoadProcess($crate::components::vector_container::ContainerLoadingProcess<F64Container, f64>, $crate::nexosim::Mailbox<$crate::components::vector_container::ContainerLoadingProcess<F64Container, f64>>),
            F64ContainerUnloadProcess($crate::components::vector_container::ContainerUnloadingProcess<F64Container, f64>, $crate::nexosim::Mailbox<$crate::components::vector_container::ContainerUnloadingProcess<F64Container, f64>>),

            Vector3ContainerStock($crate::components::discrete::DiscreteStock<Vector3Container>, $crate::nexosim::Mailbox<$crate::components::discrete::DiscreteStock<Vector3Container>>),
            Vector3ContainerProcess($crate::components::discrete::DiscreteProcess<(), Option<Vector3Container>, Vector3Container, Vector3Container>, $crate::nexosim::Mailbox<$crate::components::discrete::DiscreteProcess<(), Option<Vector3Container>, Vector3Container, Vector3Container>>),
            Vector3ContainerParallelProcess($crate::components::discrete::DiscreteParallelProcess<(), Option<Vector3Container>, Vector3Container, Vector3Container>, $crate::nexosim::Mailbox<$crate::components::discrete::DiscreteParallelProcess<(), Option<Vector3Container>, Vector3Container, Vector3Container>>),
            Vector3ContainerSource($crate::components::discrete::DiscreteSource<Vector3Container, Vector3Container, Vector3ContainerFactory>, $crate::nexosim::Mailbox<$crate::components::discrete::DiscreteSource<Vector3Container, Vector3Container, Vector3ContainerFactory>>),
            Vector3ContainerSink($crate::components::discrete::DiscreteSink<(), Option<Vector3Container>, Vector3Container>, $crate::nexosim::Mailbox<$crate::components::discrete::DiscreteSink<(), Option<Vector3Container>, Vector3Container>>),
            Vector3ContainerLoadProcess($crate::components::vector_container::ContainerLoadingProcess<Vector3Container, Vector3>, $crate::nexosim::Mailbox<$crate::components::vector_container::ContainerLoadingProcess<Vector3Container, Vector3>>),
            Vector3ContainerUnloadProcess($crate::components::vector_container::ContainerUnloadingProcess<Vector3Container, Vector3>, $crate::nexosim::Mailbox<$crate::components::vector_container::ContainerUnloadingProcess<Vector3Container, Vector3>>),
            
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
                    ($ComponentModel::F64Stock(a, ad), $ComponentModel::F64Process(b, bd), _) => {
                        a.state_emitter.connect($crate::components::vector::VectorProcess::update_state, bd.address());
                        b.req_upstream.connect($crate::components::vector::VectorStock::get_state_async, ad.address());
                        b.withdraw_upstream.connect($crate::components::vector::VectorStock::remove, ad.address());
                        Ok(())
                    },
                    ($ComponentModel::F64Process(a, ad), $ComponentModel::F64Stock(b, bd), _) => {
                        b.state_emitter.connect($crate::components::vector::VectorProcess::update_state, ad.address());
                        a.req_downstream.connect($crate::components::vector::VectorStock::get_state_async, bd.address());
                        a.push_downstream.connect($crate::components::vector::VectorStock::add, bd.address());
                        Ok(())
                    },
                    ($ComponentModel::BasicEnvironment(a, am), $ComponentModel::F64Process(b, bm), _) => {
                        a.emit_change.connect($crate::components::vector::VectorProcess::update_state, bm.address());
                        b.req_environment.connect($crate::core::BasicEnvironment::get_state_async, am.address());
                        Ok(())
                    }
                    // F64Source
                    ($ComponentModel::F64Source(a, am), $ComponentModel::F64Stock(b, bm), _) => {
                        b.state_emitter.connect($crate::components::vector::VectorSource::update_state, am.address());
                        a.req_downstream.connect($crate::components::vector::VectorStock::get_state_async, bm.address());
                        a.push_downstream.connect($crate::components::vector::VectorStock::add, bm.address());
                        Ok(())
                    },
                    // F64Sink
                    ($ComponentModel::F64Stock(a, am), $ComponentModel::F64Sink(b, bm), _) => {
                        a.state_emitter.connect($crate::components::vector::VectorSink::update_state, bm.address());
                        b.req_upstream.connect($crate::components::vector::VectorStock::get_state_async, am.address());
                        b.withdraw_upstream.connect($crate::components::vector::VectorStock::remove, am.address());
                        Ok(())
                    },

                    // F64Combiner1
                    ($ComponentModel::F64Stock(a, am), $ComponentModel::F64Combiner1(b, bm), Some(n)) => {
                        a.state_emitter.connect($crate::components::vector::VectorCombiner::update_state, bm.address());
                        b.req_upstreams[n].connect($crate::components::vector::VectorStock::get_state_async, am.address());
                        b.withdraw_upstreams[n].connect($crate::components::vector::VectorStock::remove, am.address());
                        Ok(())
                    },
                    ($ComponentModel::F64Combiner1(a, am), $ComponentModel::F64Stock(b, bm), _) => {
                        b.state_emitter.connect($crate::components::vector::VectorCombiner::update_state, am.address());
                        a.req_downstream.connect($crate::components::vector::VectorStock::get_state_async, bm.address());
                        a.push_downstream.connect($crate::components::vector::VectorStock::add, bm.address());
                        Ok(())
                    },
                    ($ComponentModel::F64Stock(a, am), $ComponentModel::F64Combiner2(b, bm), Some(n)) => {
                        a.state_emitter.connect($crate::components::vector::VectorCombiner::update_state, bm.address());
                        b.req_upstreams[n].connect($crate::components::vector::VectorStock::get_state_async, am.address());
                        b.withdraw_upstreams[n].connect($crate::components::vector::VectorStock::remove, am.address());
                        Ok(())
                    },
                    ($ComponentModel::F64Combiner2(a, am), $ComponentModel::F64Stock(b, bm), _) => {
                        b.state_emitter.connect($crate::components::vector::VectorCombiner::update_state, am.address());
                        a.req_downstream.connect($crate::components::vector::VectorStock::get_state_async, bm.address());
                        a.push_downstream.connect($crate::components::vector::VectorStock::add, bm.address());
                        Ok(())
                    },
                    ($ComponentModel::F64Stock(a, am), $ComponentModel::F64Combiner3(b, bm), Some(n)) => {
                        a.state_emitter.connect($crate::components::vector::VectorCombiner::update_state, bm.address());
                        b.req_upstreams[n].connect($crate::components::vector::VectorStock::get_state_async, am.address());
                        b.withdraw_upstreams[n].connect($crate::components::vector::VectorStock::remove, am.address());
                        Ok(())
                    },
                    ($ComponentModel::F64Combiner3(a, am), $ComponentModel::F64Stock(b, bm), _) => {
                        b.state_emitter.connect($crate::components::vector::VectorCombiner::update_state, am.address());
                        a.req_downstream.connect($crate::components::vector::VectorStock::get_state_async, bm.address());
                        a.push_downstream.connect($crate::components::vector::VectorStock::add, bm.address());
                        Ok(())
                    },
                    ($ComponentModel::F64Stock(a, am), $ComponentModel::F64Combiner4(b, bm), Some(n)) => {
                        a.state_emitter.connect($crate::components::vector::VectorCombiner::update_state, bm.address());
                        b.req_upstreams[n].connect($crate::components::vector::VectorStock::get_state_async, am.address());
                        b.withdraw_upstreams[n].connect($crate::components::vector::VectorStock::remove, am.address());
                        Ok(())
                    },
                    ($ComponentModel::F64Combiner4(a, am), $ComponentModel::F64Stock(b, bm), _) => {
                        b.state_emitter.connect($crate::components::vector::VectorCombiner::update_state, am.address());
                        a.req_downstream.connect($crate::components::vector::VectorStock::get_state_async, bm.address());
                        a.push_downstream.connect($crate::components::vector::VectorStock::add, bm.address());
                        Ok(())
                    },
                    ($ComponentModel::F64Stock(a, am), $ComponentModel::F64Combiner5(b, bm), Some(n)) => {
                        a.state_emitter.connect($crate::components::vector::VectorCombiner::update_state, bm.address());
                        b.req_upstreams[n].connect($crate::components::vector::VectorStock::get_state_async, am.address());
                        b.withdraw_upstreams[n].connect($crate::components::vector::VectorStock::remove, am.address());
                        Ok(())
                    },
                    ($ComponentModel::F64Combiner5(a, am), $ComponentModel::F64Stock(b, bm), _) => {
                        b.state_emitter.connect($crate::components::vector::VectorCombiner::update_state, am.address());
                        a.req_downstream.connect($crate::components::vector::VectorStock::get_state_async, bm.address());
                        a.push_downstream.connect($crate::components::vector::VectorStock::add, bm.address());
                        Ok(())
                    },
                    ($ComponentModel::F64Splitter1(a, am), $ComponentModel::F64Stock(b, bm), Some(n)) => {
                        b.state_emitter.connect($crate::components::vector::VectorSplitter::update_state, am.address());
                        a.req_downstreams[n].connect($crate::components::vector::VectorStock::get_state_async, bm.address());
                        a.push_downstreams[n].connect($crate::components::vector::VectorStock::add, bm.address());
                        Ok(())
                    },
                    ($ComponentModel::F64Stock(a, am), $ComponentModel::F64Splitter1(b, bm), _) => {
                        a.state_emitter.connect($crate::components::vector::VectorSplitter::update_state, bm.address());
                        b.req_upstream.connect($crate::components::vector::VectorStock::get_state_async, am.address());
                        b.withdraw_upstream.connect($crate::components::vector::VectorStock::remove, am.address());
                        Ok(())
                    },
                    ($ComponentModel::F64Splitter2(a, am), $ComponentModel::F64Stock(b, bm), Some(n)) => {
                        b.state_emitter.connect($crate::components::vector::VectorSplitter::update_state, am.address());
                        a.req_downstreams[n].connect($crate::components::vector::VectorStock::get_state_async, bm.address());
                        a.push_downstreams[n].connect($crate::components::vector::VectorStock::add, bm.address());
                        Ok(())
                    },
                    ($ComponentModel::F64Stock(a, am), $ComponentModel::F64Splitter2(b, bm), _) => {
                        a.state_emitter.connect($crate::components::vector::VectorSplitter::update_state, bm.address());
                        b.req_upstream.connect($crate::components::vector::VectorStock::get_state_async, am.address());
                        b.withdraw_upstream.connect($crate::components::vector::VectorStock::remove, am.address());
                        Ok(())
                    },
                    ($ComponentModel::F64Splitter3(a, am), $ComponentModel::F64Stock(b, bm), Some(n)) => {
                        b.state_emitter.connect($crate::components::vector::VectorSplitter::update_state, am.address());
                        a.req_downstreams[n].connect($crate::components::vector::VectorStock::get_state_async, bm.address());
                        a.push_downstreams[n].connect($crate::components::vector::VectorStock::add, bm.address());
                        Ok(())
                    },
                    ($ComponentModel::F64Stock(a, am), $ComponentModel::F64Splitter3(b, bm), _) => {
                        a.state_emitter.connect($crate::components::vector::VectorSplitter::update_state, bm.address());
                        b.req_upstream.connect($crate::components::vector::VectorStock::get_state_async, am.address());
                        b.withdraw_upstream.connect($crate::components::vector::VectorStock::remove, am.address());
                        Ok(())
                    },
                    ($ComponentModel::F64Splitter4(a, am), $ComponentModel::F64Stock(b, bm), Some(n)) => {
                        b.state_emitter.connect($crate::components::vector::VectorSplitter::update_state, am.address());
                        a.req_downstreams[n].connect($crate::components::vector::VectorStock::get_state_async, bm.address());
                        a.push_downstreams[n].connect($crate::components::vector::VectorStock::add, bm.address());
                        Ok(())
                    },
                    ($ComponentModel::F64Stock(a, am), $ComponentModel::F64Splitter4(b, bm), _) => {
                        a.state_emitter.connect($crate::components::vector::VectorSplitter::update_state, bm.address());
                        b.req_upstream.connect($crate::components::vector::VectorStock::get_state_async, am.address());
                        b.withdraw_upstream.connect($crate::components::vector::VectorStock::remove, am.address());
                        Ok(())
                    },
                    ($ComponentModel::F64Splitter5(a, am), $ComponentModel::F64Stock(b, bm), Some(n)) => {
                        b.state_emitter.connect($crate::components::vector::VectorSplitter::update_state, am.address());
                        a.req_downstreams[n].connect($crate::components::vector::VectorStock::get_state_async, bm.address());
                        a.push_downstreams[n].connect($crate::components::vector::VectorStock::add, bm.address());
                        Ok(())
                    },
                    ($ComponentModel::F64Stock(a, am), $ComponentModel::F64Splitter5(b, bm), n) => {
                        a.state_emitter.connect($crate::components::vector::VectorSplitter::update_state, bm.address());
                        b.req_upstream.connect($crate::components::vector::VectorStock::get_state_async, am.address());
                        b.withdraw_upstream.connect($crate::components::vector::VectorStock::remove, am.address());
                        Ok(())
                    },

                    ($ComponentModel::Vector3Stock(a, ad), $ComponentModel::Vector3Process(b, bd), _) => {
                        a.state_emitter.connect($crate::components::vector::VectorProcess::update_state, bd.address());
                        b.req_upstream.connect($crate::components::vector::VectorStock::get_state_async, ad.address());
                        b.withdraw_upstream.connect($crate::components::vector::VectorStock::remove, ad.address());
                        Ok(())
                    },
                    ($ComponentModel::Vector3Process(a, ad), $ComponentModel::Vector3Stock(b, bd), _) => {
                        b.state_emitter.connect($crate::components::vector::VectorProcess::update_state, ad.address());
                        a.req_downstream.connect($crate::components::vector::VectorStock::get_state_async, bd.address());
                        a.push_downstream.connect($crate::components::vector::VectorStock::add, bd.address());
                        Ok(())
                    },
                    // Vector3Source
                    ($ComponentModel::Vector3Source(a, am), $ComponentModel::Vector3Stock(b, bm), _) => {
                        b.state_emitter.connect($crate::components::vector::VectorSource::update_state, am.address());
                        a.req_downstream.connect($crate::components::vector::VectorStock::get_state_async, bm.address());
                        a.push_downstream.connect($crate::components::vector::VectorStock::add, bm.address());
                        Ok(())
                    },
                    // Vector3Sink
                    ($ComponentModel::Vector3Stock(a, am), $ComponentModel::Vector3Sink(b, bm), _) => {
                        a.state_emitter.connect($crate::components::vector::VectorSink::update_state, bm.address());
                        b.req_upstream.connect($crate::components::vector::VectorStock::get_state_async, am.address());
                        b.withdraw_upstream.connect($crate::components::vector::VectorStock::remove, am.address());
                        Ok(())
                    },

                    // Vector3Combiner1
                    ($ComponentModel::Vector3Stock(a, am), $ComponentModel::Vector3Combiner1(b, bm), Some(n)) => {
                        a.state_emitter.connect($crate::components::vector::VectorCombiner::update_state, bm.address());
                        b.req_upstreams[n].connect($crate::components::vector::VectorStock::get_state_async, am.address());
                        b.withdraw_upstreams[n].connect($crate::components::vector::VectorStock::remove, am.address());
                        Ok(())
                    },
                    ($ComponentModel::Vector3Combiner1(a, am), $ComponentModel::Vector3Stock(b, bm), _) => {
                        b.state_emitter.connect($crate::components::vector::VectorCombiner::update_state, am.address());
                        a.req_downstream.connect($crate::components::vector::VectorStock::get_state_async, bm.address());
                        a.push_downstream.connect($crate::components::vector::VectorStock::add, bm.address());
                        Ok(())
                    },
                    // Vector3Combiner2
                    ($ComponentModel::Vector3Stock(a, am), $ComponentModel::Vector3Combiner2(b, bm), Some(n)) => {
                        a.state_emitter.connect($crate::components::vector::VectorCombiner::update_state, bm.address());
                        b.req_upstreams[n].connect($crate::components::vector::VectorStock::get_state_async, am.address());
                        b.withdraw_upstreams[n].connect($crate::components::vector::VectorStock::remove, am.address());
                        Ok(())
                    },
                    ($ComponentModel::Vector3Combiner2(a, am), $ComponentModel::Vector3Stock(b, bm), _) => {
                        b.state_emitter.connect($crate::components::vector::VectorCombiner::update_state, am.address());
                        a.req_downstream.connect($crate::components::vector::VectorStock::get_state_async, bm.address());
                        a.push_downstream.connect($crate::components::vector::VectorStock::add, bm.address());
                        Ok(())
                    },
                    // Vector3Combiner3
                    ($ComponentModel::Vector3Stock(a, am), $ComponentModel::Vector3Combiner3(b, bm), Some(n)) => {
                        a.state_emitter.connect($crate::components::vector::VectorCombiner::update_state, bm.address());
                        b.req_upstreams[n].connect($crate::components::vector::VectorStock::get_state_async, am.address());
                        b.withdraw_upstreams[n].connect($crate::components::vector::VectorStock::remove, am.address());
                        Ok(())
                    },
                    ($ComponentModel::Vector3Combiner3(a, am), $ComponentModel::Vector3Stock(b, bm), _) => {
                        b.state_emitter.connect($crate::components::vector::VectorCombiner::update_state, am.address());
                        a.req_downstream.connect($crate::components::vector::VectorStock::get_state_async, bm.address());
                        a.push_downstream.connect($crate::components::vector::VectorStock::add, bm.address());
                        Ok(())
                    },
                    // Vector3Combiner4
                    ($ComponentModel::Vector3Stock(a, am), $ComponentModel::Vector3Combiner4(b, bm), Some(n)) => {
                        a.state_emitter.connect($crate::components::vector::VectorCombiner::update_state, bm.address());
                        b.req_upstreams[n].connect($crate::components::vector::VectorStock::get_state_async, am.address());
                        b.withdraw_upstreams[n].connect($crate::components::vector::VectorStock::remove, am.address());
                        Ok(())
                    },
                    ($ComponentModel::Vector3Combiner4(a, am), $ComponentModel::Vector3Stock(b, bm), _) => {
                        b.state_emitter.connect($crate::components::vector::VectorCombiner::update_state, am.address());
                        a.req_downstream.connect($crate::components::vector::VectorStock::get_state_async, bm.address());
                        a.push_downstream.connect($crate::components::vector::VectorStock::add, bm.address());
                        Ok(())
                    },
                    // Vector3Combiner5
                    ($ComponentModel::Vector3Stock(a, am), $ComponentModel::Vector3Combiner5(b, bm), Some(n)) => {
                        a.state_emitter.connect($crate::components::vector::VectorCombiner::update_state, bm.address());
                        b.req_upstreams[n].connect($crate::components::vector::VectorStock::get_state_async, am.address());
                        b.withdraw_upstreams[n].connect($crate::components::vector::VectorStock::remove, am.address());
                        Ok(())
                    },
                    ($ComponentModel::Vector3Combiner5(a, am), $ComponentModel::Vector3Stock(b, bm), _) => {
                        b.state_emitter.connect($crate::components::vector::VectorCombiner::update_state, am.address());
                        a.req_downstream.connect($crate::components::vector::VectorStock::get_state_async, bm.address());
                        a.push_downstream.connect($crate::components::vector::VectorStock::add, bm.address());
                        Ok(())
                    },

                    // Vector3Splitter1
                    ($ComponentModel::Vector3Stock(a, amb), $ComponentModel::Vector3Splitter1(b, bmb), _) => {
                        a.state_emitter.connect($crate::components::vector::VectorSplitter::update_state, bmb.address());
                        b.req_upstream.connect($crate::components::vector::VectorStock::get_state_async, amb.address());
                        b.withdraw_upstream.connect($crate::components::vector::VectorStock::remove, amb.address());
                        Ok(())
                    },
                    ($ComponentModel::Vector3Splitter1(a, amb), $ComponentModel::Vector3Stock(b, bmb), Some(n)) => {
                        b.state_emitter.connect($crate::components::vector::VectorSplitter::update_state, amb.address());
                        a.req_downstreams[n].connect($crate::components::vector::VectorStock::get_state_async, bmb.address());
                        a.push_downstreams[n].connect($crate::components::vector::VectorStock::add, bmb.address());
                        Ok(())
                    },
                    // Vector3Splitter2
                    ($ComponentModel::Vector3Stock(a, amb), $ComponentModel::Vector3Splitter2(b, bmb), _) => {
                        a.state_emitter.connect($crate::components::vector::VectorSplitter::update_state, bmb.address());
                        b.req_upstream.connect($crate::components::vector::VectorStock::get_state_async, amb.address());
                        b.withdraw_upstream.connect($crate::components::vector::VectorStock::remove, amb.address());
                        Ok(())
                    },
                    ($ComponentModel::Vector3Splitter2(a, amb), $ComponentModel::Vector3Stock(b, bmb), Some(n)) => {
                        b.state_emitter.connect($crate::components::vector::VectorSplitter::update_state, amb.address());
                        a.req_downstreams[n].connect($crate::components::vector::VectorStock::get_state_async, bmb.address());
                        a.push_downstreams[n].connect($crate::components::vector::VectorStock::add, bmb.address());
                        Ok(())
                    },
                    // Vector3Splitter3
                    ($ComponentModel::Vector3Stock(a, amb), $ComponentModel::Vector3Splitter3(b, bmb), _) => {
                        a.state_emitter.connect($crate::components::vector::VectorSplitter::update_state, bmb.address());
                        b.req_upstream.connect($crate::components::vector::VectorStock::get_state_async, amb.address());
                        b.withdraw_upstream.connect($crate::components::vector::VectorStock::remove, amb.address());
                        Ok(())
                    },
                    ($ComponentModel::Vector3Splitter3(a, amb), $ComponentModel::Vector3Stock(b, bmb), Some(n)) => {
                        b.state_emitter.connect($crate::components::vector::VectorSplitter::update_state, amb.address());
                        a.req_downstreams[n].connect($crate::components::vector::VectorStock::get_state_async, bmb.address());
                        a.push_downstreams[n].connect($crate::components::vector::VectorStock::add, bmb.address());
                        Ok(())
                    },
                    // Vector3Splitter4
                    ($ComponentModel::Vector3Stock(a, amb), $ComponentModel::Vector3Splitter4(b, bmb), _) => {
                        a.state_emitter.connect($crate::components::vector::VectorSplitter::update_state, bmb.address());
                        b.req_upstream.connect($crate::components::vector::VectorStock::get_state_async, amb.address());
                        b.withdraw_upstream.connect($crate::components::vector::VectorStock::remove, amb.address());
                        Ok(())
                    },
                    ($ComponentModel::Vector3Splitter4(a, amb), $ComponentModel::Vector3Stock(b, bmb), Some(n)) => {
                        b.state_emitter.connect($crate::components::vector::VectorSplitter::update_state, amb.address());
                        a.req_downstreams[n].connect($crate::components::vector::VectorStock::get_state_async, bmb.address());
                        a.push_downstreams[n].connect($crate::components::vector::VectorStock::add, bmb.address());
                        Ok(())
                    },
                    // Vector3Splitter5
                    ($ComponentModel::Vector3Stock(a, amb), $ComponentModel::Vector3Splitter5(b, bmb), _) => {
                        a.state_emitter.connect($crate::components::vector::VectorSplitter::update_state, bmb.address());
                        b.req_upstream.connect($crate::components::vector::VectorStock::get_state_async, amb.address());
                        b.withdraw_upstream.connect($crate::components::vector::VectorStock::remove, amb.address());
                        Ok(())
                    },
                    ($ComponentModel::Vector3Splitter5(a, amb), $ComponentModel::Vector3Stock(b, bmb), Some(n)) => {
                        b.state_emitter.connect($crate::components::vector::VectorSplitter::update_state, amb.address());
                        a.req_downstreams[n].connect($crate::components::vector::VectorStock::get_state_async, bmb.address());
                        a.push_downstreams[n].connect($crate::components::vector::VectorStock::add, bmb.address());
                        Ok(())
                    },

                    // Discrete
                    ($ComponentModel::StringStock(a, ad), $ComponentModel::StringProcess(b, bd), _) => {
                        a.state_emitter.connect($crate::components::discrete::DiscreteProcess::update_state, bd.address());
                        b.req_upstream.connect($crate::components::discrete::DiscreteStock::get_state_async, ad.address());
                        b.withdraw_upstream.connect($crate::components::discrete::DiscreteStock::remove, ad.address());
                        Ok(())
                    },
                    ($ComponentModel::StringProcess(a, ad), $ComponentModel::StringStock(b, bd), _) => {
                        b.state_emitter.connect($crate::components::discrete::DiscreteProcess::update_state, ad.address());
                        a.req_downstream.connect($crate::components::discrete::DiscreteStock::get_state_async, bd.address());
                        a.push_downstream.connect($crate::components::discrete::DiscreteStock::add, bd.address());
                        Ok(())
                    },
                    ($ComponentModel::StringStock(a, am), $ComponentModel::StringParallelProcess(b, bm), _) => {
                        a.state_emitter.connect($crate::components::discrete::DiscreteParallelProcess::update_state, bm.address());
                        b.req_upstream.connect($crate::components::discrete::DiscreteStock::get_state_async, am.address());
                        b.withdraw_upstream.connect($crate::components::discrete::DiscreteStock::remove, am.address());
                        Ok(())
                    },
                    ($ComponentModel::StringParallelProcess(a, am), $ComponentModel::StringStock(b, bm), _) => {
                        b.state_emitter.connect($crate::components::discrete::DiscreteParallelProcess::update_state, am.address());
                        a.req_downstream.connect($crate::components::discrete::DiscreteStock::get_state_async, bm.address());
                        a.push_downstream.connect($crate::components::discrete::DiscreteStock::add, bm.address());
                        Ok(())
                    },
                    ($ComponentModel::StringSource(a, am), $ComponentModel::StringStock(b, bm), _) => {
                        b.state_emitter.connect($crate::components::discrete::DiscreteSource::update_state, am.address());
                        a.req_downstream.connect($crate::components::discrete::DiscreteStock::get_state_async, bm.address());
                        a.push_downstream.connect($crate::components::discrete::DiscreteStock::add, bm.address());
                        Ok(())
                    },
                    ($ComponentModel::StringStock(a, am), $ComponentModel::StringSink(b, bm), _) => {
                        a.state_emitter.connect($crate::components::discrete::DiscreteSink::update_state, bm.address());
                        b.req_upstream.connect($crate::components::discrete::DiscreteStock::get_state_async, am.address());
                        b.withdraw_upstream.connect($crate::components::discrete::DiscreteStock::remove, am.address());
                        Ok(())
                    },

                    ($ComponentModel::Vector3ContainerStock(a, am), $ComponentModel::Vector3ContainerProcess(b, bm), _) => {
                        a.state_emitter.connect($crate::components::discrete::DiscreteProcess::update_state, bm.address());
                        b.req_upstream.connect($crate::components::discrete::DiscreteStock::get_state_async, am.address());
                        b.withdraw_upstream.connect($crate::components::discrete::DiscreteStock::remove, am.address());
                        Ok(())
                    },
                    ($ComponentModel::Vector3ContainerProcess(a, am), $ComponentModel::Vector3ContainerStock(b, bm), _) => {
                        b.state_emitter.connect($crate::components::discrete::DiscreteProcess::update_state, am.address());
                        a.req_downstream.connect($crate::components::discrete::DiscreteStock::get_state_async, bm.address());
                        a.push_downstream.connect($crate::components::discrete::DiscreteStock::add, bm.address());
                        Ok(())
                    },
                    ($ComponentModel::Vector3ContainerStock(a, am), $ComponentModel::Vector3ContainerParallelProcess(b, bm), _) => {
                        a.state_emitter.connect($crate::components::discrete::DiscreteParallelProcess::update_state, bm.address());
                        b.req_upstream.connect($crate::components::discrete::DiscreteStock::get_state_async, am.address());
                        b.withdraw_upstream.connect($crate::components::discrete::DiscreteStock::remove, am.address());
                        Ok(())
                    },
                    ($ComponentModel::Vector3ContainerParallelProcess(a, am), $ComponentModel::Vector3ContainerStock(b, bm), _) => {
                        b.state_emitter.connect($crate::components::discrete::DiscreteParallelProcess::update_state, am.address());
                        a.req_downstream.connect($crate::components::discrete::DiscreteStock::get_state_async, bm.address());
                        a.push_downstream.connect($crate::components::discrete::DiscreteStock::add, bm.address());
                        Ok(())
                    },
                    ($ComponentModel::Vector3ContainerSource(a, am), $ComponentModel::Vector3ContainerStock(b, bm), _) => {
                        b.state_emitter.connect($crate::components::discrete::DiscreteSource::<Vector3Container, Vector3Container, Vector3ContainerFactory>::update_state, am.address());
                        a.req_downstream.connect($crate::components::discrete::DiscreteStock::<Vector3Container>::get_state_async, bm.address());
                        a.push_downstream.connect($crate::components::discrete::DiscreteStock::<Vector3Container>::add, bm.address());
                        Ok(())
                    },
                    ($ComponentModel::Vector3ContainerStock(a, am), $ComponentModel::Vector3ContainerSink(b, bm), _) => {
                        a.state_emitter.connect($crate::components::discrete::DiscreteSink::<(), Option<Vector3Container>, Vector3Container>::update_state, bm.address());
                        b.req_upstream.connect($crate::components::discrete::DiscreteStock::<Vector3Container>::get_state_async, am.address());
                        b.withdraw_upstream.connect($crate::components::discrete::DiscreteStock::<Vector3Container>::remove, am.address());
                        Ok(())
                    },

                    ($ComponentModel::Vector3Stock(a, am), $ComponentModel::Vector3ContainerLoadProcess(b, bm), _) => {
                        a.state_emitter.connect($crate::components::vector_container::ContainerLoadingProcess::update_state, bm.address());
                        b.req_us_resource.connect($crate::components::vector::VectorStock::get_state_async, am.address());
                        b.withdraw_us_resource.connect($crate::components::vector::VectorStock::remove, am.address());
                        Ok(())
                    },
                    ($ComponentModel::Vector3ContainerStock(a, am), $ComponentModel::Vector3ContainerLoadProcess(b, bm), _) => {
                        a.state_emitter.connect($crate::components::vector_container::ContainerLoadingProcess::update_state, bm.address());
                        b.req_us_containers.connect($crate::components::discrete::DiscreteStock::get_state_async, am.address());
                        b.withdraw_us_containers.connect($crate::components::discrete::DiscreteStock::remove, am.address());
                        Ok(())
                    },
                    ($ComponentModel::Vector3ContainerLoadProcess(a, am), $ComponentModel::Vector3ContainerStock(b, bm), _) => {
                        b.state_emitter.connect($crate::components::vector_container::ContainerLoadingProcess::update_state, am.address());
                        a.req_downstream.connect($crate::components::discrete::DiscreteStock::get_state_async, bm.address());
                        a.push_downstream.connect($crate::components::discrete::DiscreteStock::add, bm.address());
                        Ok(())
                    },
                    ($ComponentModel::Vector3ContainerStock(a, am), $ComponentModel::Vector3ContainerUnloadProcess(b, bm), _) => {
                        a.state_emitter.connect($crate::components::vector_container::ContainerUnloadingProcess::update_state, bm.address());
                        b.req_upstream.connect($crate::components::discrete::DiscreteStock::get_state_async, am.address());
                        b.withdraw_upstream.connect($crate::components::discrete::DiscreteStock::remove, am.address());
                        Ok(())
                    },
                    ($ComponentModel::Vector3ContainerUnloadProcess(a, am), $ComponentModel::Vector3ContainerStock(b, bm), _) => {
                        b.state_emitter.connect($crate::components::vector_container::ContainerUnloadingProcess::update_state, am.address());
                        a.req_ds_containers.connect($crate::components::discrete::DiscreteStock::get_state_async, bm.address());
                        a.push_ds_containers.connect($crate::components::discrete::DiscreteStock::add, bm.address());
                        Ok(())
                    },
                    ($ComponentModel::Vector3ContainerUnloadProcess(a, am), $ComponentModel::Vector3Stock(b, bm), _) => {
                        b.state_emitter.connect($crate::components::vector_container::ContainerUnloadingProcess::update_state, am.address());
                        a.req_ds_resource.connect($crate::components::vector::VectorStock::get_state_async, bm.address());
                        a.push_ds_resource.connect($crate::components::vector::VectorStock::add, bm.address());
                        Ok(())
                    },
                    /**
                     * Environments
                     */
                    ($ComponentModel::BasicEnvironment(a, am), $ComponentModel::F64Process(b, bm), _) => {
                        a.emit_change.connect($crate::components::vector::VectorProcess::update_state, bm.address());
                        b.req_environment.connect($crate::core::BasicEnvironment::get_state_async, am.address());
                        Ok(())
                    },
                    ($ComponentModel::BasicEnvironment(a, am), $ComponentModel::F64Source(b, bm), _) => {
                        a.emit_change.connect($crate::components::vector::VectorSource::update_state, bm.address());
                        b.req_environment.connect($crate::core::BasicEnvironment::get_state_async, am.address());
                        Ok(())
                    },
                    ($ComponentModel::BasicEnvironment(a, am), $ComponentModel::F64Sink(b, bm), _) => {
                        a.emit_change.connect($crate::components::vector::VectorSink::update_state, bm.address());
                        b.req_environment.connect($crate::core::BasicEnvironment::get_state_async, am.address());
                        Ok(())
                    },
                    ($ComponentModel::BasicEnvironment(a, am), $ComponentModel::F64Combiner1(b, bm), _) => {
                        a.emit_change.connect($crate::components::vector::VectorCombiner::update_state, bm.address());
                        b.req_environment.connect($crate::core::BasicEnvironment::get_state_async, am.address());
                        Ok(())
                    },
                    ($ComponentModel::BasicEnvironment(a, am), $ComponentModel::F64Combiner2(b, bm), _) => {
                        a.emit_change.connect($crate::components::vector::VectorCombiner::update_state, bm.address());
                        b.req_environment.connect($crate::core::BasicEnvironment::get_state_async, am.address());
                        Ok(())
                    },
                    ($ComponentModel::BasicEnvironment(a, am), $ComponentModel::F64Combiner3(b, bm), _) => {
                        a.emit_change.connect($crate::components::vector::VectorCombiner::update_state, bm.address());
                        b.req_environment.connect($crate::core::BasicEnvironment::get_state_async, am.address());
                        Ok(())
                    },
                    ($ComponentModel::BasicEnvironment(a, am), $ComponentModel::F64Combiner4(b, bm), _) => {
                        a.emit_change.connect($crate::components::vector::VectorCombiner::update_state, bm.address());
                        b.req_environment.connect($crate::core::BasicEnvironment::get_state_async, am.address());
                        Ok(())
                    },
                    ($ComponentModel::BasicEnvironment(a, am), $ComponentModel::F64Combiner5(b, bm), _) => {
                        a.emit_change.connect($crate::components::vector::VectorCombiner::update_state, bm.address());
                        b.req_environment.connect($crate::core::BasicEnvironment::get_state_async, am.address());
                        Ok(())
                    },
                    ($ComponentModel::BasicEnvironment(a, am), $ComponentModel::F64Splitter1(b, bm), _) => {
                        a.emit_change.connect($crate::components::vector::VectorSplitter::update_state, bm.address());
                        b.req_environment.connect($crate::core::BasicEnvironment::get_state_async, am.address());
                        Ok(())
                    },
                    ($ComponentModel::BasicEnvironment(a, am), $ComponentModel::F64Splitter2(b, bm), _) => {
                        a.emit_change.connect($crate::components::vector::VectorSplitter::update_state, bm.address());
                        b.req_environment.connect($crate::core::BasicEnvironment::get_state_async, am.address());
                        Ok(())
                    },
                    ($ComponentModel::BasicEnvironment(a, am), $ComponentModel::F64Splitter3(b, bm), _) => {
                        a.emit_change.connect($crate::components::vector::VectorSplitter::update_state, bm.address());
                        b.req_environment.connect($crate::core::BasicEnvironment::get_state_async, am.address());
                        Ok(())
                    },
                    ($ComponentModel::BasicEnvironment(a, am), $ComponentModel::F64Splitter4(b, bm), _) => {
                        a.emit_change.connect($crate::components::vector::VectorSplitter::update_state, bm.address());
                        b.req_environment.connect($crate::core::BasicEnvironment::get_state_async, am.address());
                        Ok(())
                    },
                    ($ComponentModel::BasicEnvironment(a, am), $ComponentModel::F64Splitter5(b, bm), _) => {
                        a.emit_change.connect($crate::components::vector::VectorSplitter::update_state, bm.address());
                        b.req_environment.connect($crate::core::BasicEnvironment::get_state_async, am.address());
                        Ok(())
                    },
                    ($ComponentModel::BasicEnvironment(a, am), $ComponentModel::Vector3Process(b, bm), _) => {
                        a.emit_change.connect($crate::components::vector::VectorProcess::update_state, bm.address());
                        b.req_environment.connect($crate::core::BasicEnvironment::get_state_async, am.address());
                        Ok(())
                    },
                    ($ComponentModel::BasicEnvironment(a, am), $ComponentModel::Vector3Source(b, bm), _) => {
                        a.emit_change.connect($crate::components::vector::VectorSource::update_state, bm.address());
                        b.req_environment.connect($crate::core::BasicEnvironment::get_state_async, am.address());
                        Ok(())
                    },
                    ($ComponentModel::BasicEnvironment(a, am), $ComponentModel::Vector3Sink(b, bm), _) => {
                        a.emit_change.connect($crate::components::vector::VectorSink::update_state, bm.address());
                        b.req_environment.connect($crate::core::BasicEnvironment::get_state_async, am.address());
                        Ok(())
                    },
                    ($ComponentModel::BasicEnvironment(a, am), $ComponentModel::Vector3Combiner1(b, bm), _) => {
                        a.emit_change.connect($crate::components::vector::VectorCombiner::update_state, bm.address());
                        b.req_environment.connect($crate::core::BasicEnvironment::get_state_async, am.address());
                        Ok(())
                    },
                    ($ComponentModel::BasicEnvironment(a, am), $ComponentModel::Vector3Combiner2(b, bm), _) => {
                        a.emit_change.connect($crate::components::vector::VectorCombiner::update_state, bm.address());
                        b.req_environment.connect($crate::core::BasicEnvironment::get_state_async, am.address());
                        Ok(())
                    },
                    ($ComponentModel::BasicEnvironment(a, am), $ComponentModel::Vector3Combiner3(b, bm), _) => {
                        a.emit_change.connect($crate::components::vector::VectorCombiner::update_state, bm.address());
                        b.req_environment.connect($crate::core::BasicEnvironment::get_state_async, am.address());
                        Ok(())
                    },
                    ($ComponentModel::BasicEnvironment(a, am), $ComponentModel::Vector3Combiner4(b, bm), _) => {
                        a.emit_change.connect($crate::components::vector::VectorCombiner::update_state, bm.address());
                        b.req_environment.connect($crate::core::BasicEnvironment::get_state_async, am.address());
                        Ok(())
                    },
                    ($ComponentModel::BasicEnvironment(a, am), $ComponentModel::Vector3Combiner5(b, bm), _) => {
                        a.emit_change.connect($crate::components::vector::VectorCombiner::update_state, bm.address());
                        b.req_environment.connect($crate::core::BasicEnvironment::get_state_async, am.address());
                        Ok(())
                    },
                    ($ComponentModel::BasicEnvironment(a, am), $ComponentModel::Vector3Splitter1(b, bm), _) => {
                        a.emit_change.connect($crate::components::vector::VectorSplitter::update_state, bm.address());
                        b.req_environment.connect($crate::core::BasicEnvironment::get_state_async, am.address());
                        Ok(())
                    },
                    ($ComponentModel::BasicEnvironment(a, am), $ComponentModel::Vector3Splitter2(b, bm), _) => {
                        a.emit_change.connect($crate::components::vector::VectorSplitter::update_state, bm.address());
                        b.req_environment.connect($crate::core::BasicEnvironment::get_state_async, am.address());
                        Ok(())
                    },
                    ($ComponentModel::BasicEnvironment(a, am), $ComponentModel::Vector3Splitter3(b, bm), _) => {
                        a.emit_change.connect($crate::components::vector::VectorSplitter::update_state, bm.address());
                        b.req_environment.connect($crate::core::BasicEnvironment::get_state_async, am.address());
                        Ok(())
                    },
                    ($ComponentModel::BasicEnvironment(a, am), $ComponentModel::Vector3Splitter4(b, bm), _) => {
                        a.emit_change.connect($crate::components::vector::VectorSplitter::update_state, bm.address());
                        b.req_environment.connect($crate::core::BasicEnvironment::get_state_async, am.address());
                        Ok(())
                    },
                    ($ComponentModel::BasicEnvironment(a, am), $ComponentModel::Vector3Splitter5(b, bm), _) => {
                        a.emit_change.connect($crate::components::vector::VectorSplitter::update_state, bm.address());
                        b.req_environment.connect($crate::core::BasicEnvironment::get_state_async, am.address());
                        Ok(())
                    },
                    ($ComponentModel::BasicEnvironment(a, am), $ComponentModel::StringProcess(b, bm), _) => {
                        a.emit_change.connect($crate::components::discrete::DiscreteProcess::update_state, bm.address());
                        b.req_environment.connect($crate::core::BasicEnvironment::get_state_async, am.address());
                        Ok(())
                    },
                    ($ComponentModel::BasicEnvironment(a, am), $ComponentModel::StringSource(b, bm), _) => {
                        a.emit_change.connect($crate::components::discrete::DiscreteSource::update_state, bm.address());
                        b.req_environment.connect($crate::core::BasicEnvironment::get_state_async, am.address());
                        Ok(())
                    },
                    ($ComponentModel::BasicEnvironment(a, am), $ComponentModel::StringSink(b, bm), _) => {
                        a.emit_change.connect($crate::components::discrete::DiscreteSink::<(), Option<String>, String>::update_state, bm.address());
                        b.req_environment.connect($crate::core::BasicEnvironment::get_state_async, am.address());
                        Ok(())
                    },
                    ($ComponentModel::BasicEnvironment(a, am), $ComponentModel::StringParallelProcess(b, bm), _) => {
                        a.emit_change.connect($crate::components::discrete::DiscreteParallelProcess::<(), Option<String>, String, String>::update_state, bm.address());
                        b.req_environment.connect($crate::core::BasicEnvironment::get_state_async, am.address());
                        Ok(())
                    },
                    ($ComponentModel::BasicEnvironment(a, am), $ComponentModel::Vector3ContainerProcess(b, bm), _) => {
                        a.emit_change.connect($crate::components::discrete::DiscreteProcess::<(), Option<Vector3Container>, Vector3Container, Vector3Container>::update_state, bm.address());
                        b.req_environment.connect($crate::core::BasicEnvironment::get_state_async, am.address());
                        Ok(())
                    },
                    ($ComponentModel::BasicEnvironment(a, am), $ComponentModel::Vector3ContainerSource(b, bm), _) => {
                        a.emit_change.connect($crate::components::discrete::DiscreteSource::<Vector3Container, Vector3Container, Vector3ContainerFactory>::update_state, bm.address());
                        b.req_environment.connect($crate::core::BasicEnvironment::get_state_async, am.address());
                        Ok(())
                    },
                    ($ComponentModel::BasicEnvironment(a, am), $ComponentModel::Vector3ContainerSink(b, bm), _) => {
                        a.emit_change.connect($crate::components::discrete::DiscreteSink::<(), Option<Vector3Container>, Vector3Container>::update_state, bm.address());
                        b.req_environment.connect($crate::core::BasicEnvironment::get_state_async, am.address());
                        Ok(())
                    },
                    ($ComponentModel::BasicEnvironment(a, am), $ComponentModel::Vector3ContainerLoadProcess(b, bm), _) => {
                        a.emit_change.connect($crate::components::vector_container::ContainerLoadingProcess::<Vector3Container, Vector3>::update_state, bm.address());
                        b.req_environment.connect($crate::core::BasicEnvironment::get_state_async, am.address());
                        Ok(())
                    },
                    ($ComponentModel::BasicEnvironment(a, am), $ComponentModel::Vector3ContainerUnloadProcess(b, bm), _) => {
                        a.emit_change.connect($crate::components::vector_container::ContainerUnloadingProcess::<Vector3Container, Vector3>::update_state, bm.address());
                        b.req_environment.connect($crate::core::BasicEnvironment::get_state_async, am.address());
                        Ok(())
                    },
                    (a,b,n) => {
                        <$ComponentModel as CustomComponentConnection>::connect_components(a, b, n)
                    }
                }
            }

            pub fn register_component(mut sim_init: $crate::nexosim::SimInit, component: Self) -> $crate::nexosim::SimInit {
                use $crate::register_component_arms;
                register_component_arms!(component, sim_init,
                    F64Process, F64Stock, F64Source, F64Sink,
                    F64Combiner1, F64Combiner2, F64Combiner3, F64Combiner4, F64Combiner5,
                    F64Splitter1, F64Splitter2, F64Splitter3, F64Splitter4, F64Splitter5,
                    Vector3Stock, Vector3Process, Vector3Source, Vector3Sink,
                    Vector3Combiner1, Vector3Combiner2, Vector3Combiner3, Vector3Combiner4, Vector3Combiner5,
                    Vector3Splitter1, Vector3Splitter2, Vector3Splitter3, Vector3Splitter4, Vector3Splitter5,
                    StringStock, StringProcess, StringParallelProcess, StringSource, StringSink,
                    Vector3ContainerStock, Vector3ContainerProcess, Vector3ContainerParallelProcess,
                    Vector3ContainerSource, Vector3ContainerSink, Vector3ContainerLoadProcess, Vector3ContainerUnloadProcess,
                    BasicEnvironment,
                    $($R),*
                );
                sim_init
            }

            pub fn get_address(&self) -> $ComponentModelAddress {
                use $crate::get_address_arms;

                get_address_arms!(self,
                    F64Process, F64Stock, F64Source, F64Sink,
                    F64Combiner1, F64Combiner2, F64Combiner3, F64Combiner4, F64Combiner5,
                    F64Splitter1, F64Splitter2, F64Splitter3, F64Splitter4, F64Splitter5,
                    Vector3Stock, Vector3Process, Vector3Source, Vector3Sink,
                    Vector3Combiner1, Vector3Combiner2, Vector3Combiner3, Vector3Combiner4, Vector3Combiner5,
                    Vector3Splitter1, Vector3Splitter2, Vector3Splitter3, Vector3Splitter4, Vector3Splitter5,
                    StringStock, StringProcess, StringParallelProcess, StringSource, StringSink,
                    Vector3ContainerStock, Vector3ContainerProcess, Vector3ContainerParallelProcess,
                    Vector3ContainerSource, Vector3ContainerSink, Vector3ContainerLoadProcess, Vector3ContainerUnloadProcess,
                    BasicEnvironment,
                    $($R),*
                )
            }
        }

        #[derive(Display)]
        pub enum $ComponentModelAddress {
            F64Stock($crate::nexosim::Address<$crate::components::vector::VectorStock<f64>>),
            F64Process($crate::nexosim::Address<$crate::components::vector::VectorProcess<f64, f64, f64, f64>>),
            F64Source($crate::nexosim::Address<$crate::components::vector::VectorSource<f64, f64>>),
            F64Sink($crate::nexosim::Address<$crate::components::vector::VectorSink<f64, f64, f64>>),
            F64Combiner1($crate::nexosim::Address<$crate::components::vector::VectorCombiner<f64, f64, [f64; 1], f64, 1>>),
            F64Combiner2($crate::nexosim::Address<$crate::components::vector::VectorCombiner<f64, f64, [f64; 2], f64, 2>>),
            F64Combiner3($crate::nexosim::Address<$crate::components::vector::VectorCombiner<f64, f64, [f64; 3], f64, 3>>),
            F64Combiner4($crate::nexosim::Address<$crate::components::vector::VectorCombiner<f64, f64, [f64; 4], f64, 4>>),
            F64Combiner5($crate::nexosim::Address<$crate::components::vector::VectorCombiner<f64, f64, [f64; 5], f64, 5>>),
            F64Splitter1($crate::nexosim::Address<$crate::components::vector::VectorSplitter<f64, f64, f64, f64, 1>>),
            F64Splitter2($crate::nexosim::Address<$crate::components::vector::VectorSplitter<f64, f64, f64, f64, 2>>),
            F64Splitter3($crate::nexosim::Address<$crate::components::vector::VectorSplitter<f64, f64, f64, f64, 3>>),
            F64Splitter4($crate::nexosim::Address<$crate::components::vector::VectorSplitter<f64, f64, f64, f64, 4>>),
            F64Splitter5($crate::nexosim::Address<$crate::components::vector::VectorSplitter<f64, f64, f64, f64, 5>>),
            
            Vector3Stock($crate::nexosim::Address<$crate::components::vector::VectorStock<Vector3>>),
            Vector3Process($crate::nexosim::Address<$crate::components::vector::VectorProcess<f64, Vector3, Vector3, Vector3>>),
            Vector3Source($crate::nexosim::Address<$crate::components::vector::VectorSource<Vector3, Vector3>>),
            Vector3Sink($crate::nexosim::Address<$crate::components::vector::VectorSink<f64, Vector3, Vector3>>),
            Vector3Combiner1($crate::nexosim::Address<$crate::components::vector::VectorCombiner<f64, Vector3, [Vector3; 1], Vector3, 1>>),
            Vector3Combiner2($crate::nexosim::Address<$crate::components::vector::VectorCombiner<f64, Vector3, [Vector3; 2], Vector3, 2>>),
            Vector3Combiner3($crate::nexosim::Address<$crate::components::vector::VectorCombiner<f64, Vector3, [Vector3; 3], Vector3, 3>>),
            Vector3Combiner4($crate::nexosim::Address<$crate::components::vector::VectorCombiner<f64, Vector3, [Vector3; 4], Vector3, 4>>),
            Vector3Combiner5($crate::nexosim::Address<$crate::components::vector::VectorCombiner<f64, Vector3, [Vector3; 5], Vector3, 5>>),
            Vector3Splitter1($crate::nexosim::Address<$crate::components::vector::VectorSplitter<f64, Vector3, Vector3, Vector3, 1>>),
            Vector3Splitter2($crate::nexosim::Address<$crate::components::vector::VectorSplitter<f64, Vector3, Vector3, Vector3, 2>>),
            Vector3Splitter3($crate::nexosim::Address<$crate::components::vector::VectorSplitter<f64, Vector3, Vector3, Vector3, 3>>),
            Vector3Splitter4($crate::nexosim::Address<$crate::components::vector::VectorSplitter<f64, Vector3, Vector3, Vector3, 4>>),
            Vector3Splitter5($crate::nexosim::Address<$crate::components::vector::VectorSplitter<f64, Vector3, Vector3, Vector3, 5>>),
            
            StringStock($crate::nexosim::Address<$crate::components::discrete::DiscreteStock<String>>),
            StringProcess($crate::nexosim::Address<$crate::components::discrete::DiscreteProcess<(), Option<String>, String, String>>),
            StringParallelProcess($crate::nexosim::Address<$crate::components::discrete::DiscreteParallelProcess<(), Option<String>, String, String>>),
            StringSource($crate::nexosim::Address<$crate::components::discrete::DiscreteSource<String, String, StringItemFactory>>),
            StringSink($crate::nexosim::Address<$crate::components::discrete::DiscreteSink<(), Option<String>, String>>),

            F64ContainerLoadProcess($crate::nexosim::Address<$crate::components::vector_container::ContainerLoadingProcess<F64Container, f64>>),
            F64ContainerUnloadProcess($crate::nexosim::Address<$crate::components::vector_container::ContainerUnloadingProcess<F64Container, f64>>),
            F64ContainerStock($crate::nexosim::Address<$crate::components::discrete::DiscreteStock<F64Container>>),
            F64ContainerProcess($crate::nexosim::Address<$crate::components::discrete::DiscreteProcess<(), Option<F64Container>, F64Container, F64Container>>),
            F64ContainerParallelProcess($crate::nexosim::Address<$crate::components::discrete::DiscreteParallelProcess<(), Option<F64Container>, F64Container, F64Container>>),
            F64ContainerSource($crate::nexosim::Address<$crate::components::discrete::DiscreteSource<F64Container, F64Container, F64ContainerFactory>>),
            F64ContainerSink($crate::nexosim::Address<$crate::components::discrete::DiscreteSink<(), Option<F64Container>, F64Container>>),

            Vector3ContainerLoadProcess($crate::nexosim::Address<$crate::components::vector_container::ContainerLoadingProcess<Vector3Container, Vector3>>),
            Vector3ContainerUnloadProcess($crate::nexosim::Address<$crate::components::vector_container::ContainerUnloadingProcess<Vector3Container, Vector3>>),
            Vector3ContainerStock($crate::nexosim::Address<$crate::components::discrete::DiscreteStock<Vector3Container>>),
            Vector3ContainerProcess($crate::nexosim::Address<$crate::components::discrete::DiscreteProcess<(), Option<Vector3Container>, Vector3Container, Vector3Container>>),
            Vector3ContainerParallelProcess($crate::nexosim::Address<$crate::components::discrete::DiscreteParallelProcess<(), Option<Vector3Container>, Vector3Container, Vector3Container>>),
            Vector3ContainerSource($crate::nexosim::Address<$crate::components::discrete::DiscreteSource<Vector3Container, Vector3Container, Vector3ContainerFactory>>),
            Vector3ContainerSink($crate::nexosim::Address<$crate::components::discrete::DiscreteSink<(), Option<Vector3Container>, Vector3Container>>),

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

            Vector3StockLogger($crate::components::vector::VectorStockLogger<Vector3>),
            Vector3ProcessLogger($crate::components::vector::VectorProcessLogger<Vector3>),

            StringStockLogger($crate::components::discrete::DiscreteStockLogger<String>),
            StringProcessLogger($crate::components::discrete::DiscreteProcessLogger<String>),

            F64ContainerStockLogger($crate::components::discrete::DiscreteStockLogger<F64Container>),
            F64ContainerProcessLogger($crate::components::discrete::DiscreteProcessLogger<F64Container>),

            Vector3ContainerStockLogger($crate::components::discrete::DiscreteStockLogger<Vector3Container>),
            Vector3ContainerProcessLogger($crate::components::discrete::DiscreteProcessLogger<Vector3Container>),

            BasicEnvironmentLogger(BasicEnvironmentLogger),
            $(
                $(#[$logger_var_meta])*
                $U $( ( $UT ) )?
            ),*
        }

        impl $ComponentLogger {
            pub fn connect_logger(a: &mut $ComponentLogger, b: &mut $ComponentModel, n: Option<usize>) -> Result<(), Box<dyn ::std::error::Error>> {
                use $crate::core::CustomLoggerConnection;
                use $crate::connect_logger_arms;

                connect_logger_arms!(a, b, n,
                    VectorStockLoggerF64 => [F64Stock],
                    VectorProcessLoggerF64 => [F64Process, F64Source, F64Sink,
                        F64Combiner1, F64Combiner2, F64Combiner3, F64Combiner4, F64Combiner5,
                        F64Splitter1, F64Splitter2, F64Splitter3, F64Splitter4, F64Splitter5
                    ],
                    
                    Vector3StockLogger => [Vector3Stock],
                    Vector3ProcessLogger => [
                        Vector3Process, Vector3Source, Vector3Sink,
                        Vector3Combiner1, Vector3Combiner2, Vector3Combiner3, Vector3Combiner4, Vector3Combiner5,
                        Vector3Splitter1, Vector3Splitter2, Vector3Splitter3, Vector3Splitter4, Vector3Splitter5
                    ],
                    
                    StringStockLogger => [StringStock],
                    StringProcessLogger => [StringProcess, StringParallelProcess, StringSource, StringSink],
                    
                    F64ContainerStockLogger => [F64ContainerStock],
                    F64ContainerProcessLogger => [
                        F64ContainerProcess, F64ContainerParallelProcess, 
                        F64ContainerLoadProcess, F64ContainerUnloadProcess
                    ],

                    Vector3ContainerStockLogger => [Vector3ContainerStock],
                    Vector3ContainerProcessLogger => [
                        Vector3ContainerProcess, Vector3ContainerParallelProcess, 
                        Vector3ContainerLoadProcess, Vector3ContainerUnloadProcess
                    ],
                    
                    BasicEnvironmentLogger => [BasicEnvironment],
                )
            }

            pub fn write_csv(mut self, dir: &str) -> Result<(), Box<dyn Error>> {
                match self {
                    $ComponentLogger::VectorStockLoggerF64(a) => { a.write_csv(dir.to_string()) },
                    $ComponentLogger::VectorProcessLoggerF64(a) => { a.write_csv(dir.to_string()) },

                    $ComponentLogger::Vector3StockLogger(a) => { a.write_csv(dir.to_string()) },
                    $ComponentLogger::Vector3ProcessLogger(a) => { a.write_csv(dir.to_string()) },

                    $ComponentLogger::StringStockLogger(a) => { a.write_csv(dir.to_string()) },
                    $ComponentLogger::StringProcessLogger(a) => { a.write_csv(dir.to_string()) },

                    $ComponentLogger::Vector3ContainerStockLogger(a) => { a.write_csv(dir.to_string()) },
                    $ComponentLogger::Vector3ContainerProcessLogger(a) => { a.write_csv(dir.to_string()) },

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
                    ($ScheduledEventConfig::SetLowCapacity(low_capacity), $ComponentModelAddress::F64Stock(addr)) => {
                        scheduler.schedule_event(time, $crate::components::vector::VectorStock::<f64>::with_low_capacity_inplace, low_capacity.clone(), addr.clone())?;
                        scheduler.schedule_event(time, $crate::components::vector::VectorStock::<f64>::add, (0., source_event_id), addr.clone())?; // To trigger state change checks etc.
                        Ok(())
                    },
                    ($ScheduledEventConfig::SetProcessQuantity(distr), $ComponentModelAddress::F64Process(addr)) => {
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
