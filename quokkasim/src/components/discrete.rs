use std::time::Duration;
use std::fmt::Debug;

use nexosim::ports::Output;
use serde::Serialize;
use crate::prelude::*;

#[derive(Serialize, Clone, Debug)]
pub enum DiscStockLogType<T> {
    Add { balance: u32, added: Vec<T> },
    Remove { balance: u32, removed: Vec<T> },
    StateChange { new_state: DiscStockState },
}

// #[derive(Serialize, Clone, Debug)]
// enum DiscStockState {
//     Full { occupied: u32, empty: u32 },
//     Normal { occupied: u32, empty: u32 },
//     Empty { occupied: u32, empty: u32 },
// }

// impl StockState for DiscStockState {
//     fn is_same_state(&self, other: &Self) -> bool {
//         match (self, other) {
//             (DiscStockState::Full { .. }, DiscStockState::Full { .. }) => true,
//             (DiscStockState::Normal { .. }, DiscStockState::Normal { .. }) => true,
//             (DiscStockState::Empty { .. }, DiscStockState::Empty { .. }) => true,
//             _ => false,
//         }
//     }
// }

#[derive(WithMethods)]
pub struct DefaultDiscStock<ItemType, S: StockState, RecordLogType: Clone + Send + 'static> {

    // Identification
    pub element_name: String,
    pub element_code: String,
    pub element_type: String,

    // Ports
    pub log_emitter: Output<RecordLogType>,
    pub state_emitter: Output<EventId>,

    // Configuration
    pub low_capacity: u32,
    pub max_capacity: u32,

    // Runtime State
    pub resources: VecDequeStock<ItemType>,

    // Internals
    prev_state: Option<S>,
    next_event_index: u64,
}

// impl<
//     ItemType: Clone + Send + 'static,
//     LogRecordType: Clone + Send + 'static,
// > DiscStock<
//     ItemType,
//     DiscreteStockState,
//     LogRecordType,
//     DiscreteStockLogType<ItemType>,
// > for DefaultDiscStock<ItemType, DiscreteStockState, LogRecordType> 
// where 

impl<
    ItemType: Clone + Debug + Serialize + Send + 'static,
    LogRecordType: Clone + Send + 'static,
> Model for DefaultDiscStock<
    ItemType,
    DiscStockState,
    LogRecordType,
> {}

// where
//     ItemType: Clone + Debug + Serialize + Send + 'static,
//     DiscStockLogType<ItemType>: Serialize + Clone,
//     Self: ToLogRecord<DiscStockLogType<ItemType>, DiscStockLogType<ItemType>>,
// {
//     fn init(
//         mut self,
//         ctx: &mut Context<Self>,
//     ) -> impl Future<Output = InitializedModel<Self>> + Send {
//         async move {
//             let source_event_id = EventId(format!(
//                 "{}_{:06}",
//                 self.element_code, self.next_event_index
//             ));
//             self.update_state(source_event_id, ctx).await;
//             self.into()
//         }
//     }
// }

impl<T: 'static, S: StockState + Send + 'static, RecordLogType: Clone + Send + 'static> Default for DefaultDiscStock<
    T,
    S, 
    RecordLogType,
> {
    fn default() -> Self {
        DefaultDiscStock {
            element_name: "DefaultDiscStock".into(),
            element_code: "".into(),
            element_type: "DefaultDiscStock".into(),

            log_emitter: Output::default(),
            state_emitter: Output::default(),

            low_capacity: 0,
            max_capacity: u32::MAX,

            resources: VecDequeStock::new(VecDequeAccess::FIFO),

            prev_state: None,
            next_event_index: 0,
        }
    }
}

// impl<ItemType: Clone + Debug + Serialize + Send + 'static> Model for DefaultDiscProcess<
//     ItemType, 
//     DiscProcessLog<DefaultDiscProcessLogType<ItemType>, ItemType>
// >
// where
//     DiscProcessLog<DefaultDiscProcessLogType<ItemType>, ItemType>: Serialize,
//     Self: ToLogRecord<
//         DefaultDiscProcessLogType<ItemType>,
//         DiscProcessLog<DefaultDiscProcessLogType<ItemType>, ItemType>
//     > {

impl<
    ItemType: Clone + Debug + Serialize + Send + 'static,
    LogRecordType: Clone + Send + 'static,
> DiscStock<
    ItemType,
    DiscStockState,
    LogRecordType,
    DiscStockLogType<ItemType>,
> for DefaultDiscStock<ItemType, DiscStockState, LogRecordType> 
where 
    DiscStockLogType<ItemType>: Serialize,
    Self: ToLogRecord<DiscStockLogType<ItemType>, LogRecordType>,
{
    fn get_state(&mut self) -> DiscStockState {
        let occupied = self.resources.total();
        let empty = self.max_capacity.saturating_sub(occupied);
        if occupied == 0 {
            DiscStockState::Empty { occupied, empty }
        } else if empty == 0 {
            DiscStockState::Full { occupied, empty }
        } else {
            DiscStockState::Normal { occupied, empty }
        }
    }

    fn resources(&mut self) -> &mut VecDequeStock<ItemType> {
        &mut self.resources
    }

    fn log_emitter(&mut self) -> &mut Output<LogRecordType> {
        &mut self.log_emitter
    }

    fn get_next_event_id(&mut self) -> EventId {
        let event_id = EventId(format!(
            "{}_{:06}",
            self.element_code, self.next_event_index
        ));
        self.next_event_index += 1;
        event_id
    }

    fn previous_state(&mut self) -> &mut Option<DiscStockState> {
        &mut self.prev_state
    }
    fn state_emitter(&mut self) -> &mut Output<EventId> {
        &mut self.state_emitter
    }
    fn log_type_add(&self, balance: u32, resource: ItemType) -> DiscStockLogType<ItemType> {
        DiscStockLogType::Add { balance, added: vec![resource] }
    }
    fn log_type_remove(&self, balance: u32, resource: Option<ItemType>) -> DiscStockLogType<ItemType> {
        if resource.is_none() {
            return DiscStockLogType::Remove { balance, removed: vec![] };
        } else {
            return DiscStockLogType::Remove { balance, removed: vec![resource.unwrap()] };
        }
    }
    fn log_type_state_change(&self, new_state: DiscStockState) -> DiscStockLogType<ItemType> {
        DiscStockLogType::StateChange { new_state }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DiscStockLog<ItemType> {
    pub time: String,
    pub event_id: EventId,
    pub source_event_id: EventId,
    pub element_name: String,
    pub element_type: String,
    pub details: DiscStockLogType<ItemType>,

    // pub phantom: std::marker::PhantomData<ItemType>,
}

impl<ItemType> DiscStockLog<ItemType> {
    fn to_log(
        time: MonotonicTime,
        event_id: EventId,
        source_event_id: EventId,
        element_name: String,
        element_type: String,
        details: DiscStockLogType<ItemType>,
    ) -> Self {
        DiscStockLog {
            time: time.to_chrono_date_time(0).unwrap().to_string(),
            event_id,
            source_event_id,
            element_name,
            element_type,
            details,
            // phantom: std::marker::PhantomData,
        }
    }
}

impl<ItemType> ToLogRecord<
        // DefaultDiscProcessLogType<ItemType>,
        // DiscProcessLog<DefaultDiscProcessLogType<ItemType>, ItemType>,
        DiscStockLogType<ItemType>,
        DiscStockLog<ItemType>,
    > for DefaultDiscStock<
        ItemType,
        DiscStockState,
        DiscStockLog<ItemType>,
    >
where
    ItemType: Clone + Debug + Serialize + Send + 'static,
{
    fn to_record(
        &mut self,
        now: MonotonicTime,
        source_event_id: EventId,
        event_id: EventId,
        details: DiscStockLogType<ItemType>,
    ) -> DiscStockLog<ItemType> {
        DiscStockLog {
            time: now.to_chrono_date_time(0).unwrap().to_string(),
            event_id,
            source_event_id,
            element_name: self.element_name.clone(),
            element_type: self.element_type.clone(),
            details,
            // phantom: std::marker::PhantomData,
        }
    }
}


#[derive(WithMethods)]
pub struct DefaultDiscProcess<
    ItemType,
    ProcessLog,
> where
    ItemType: Clone + Debug + Serialize + Send + 'static,
    ProcessLog: Clone + Debug + Serialize + Send + 'static,
{
    // Identification
    pub element_name: String,
    pub element_code: String,
    pub element_type: String,

    // Ports
    pub req_upstream: Requestor<(), DiscStockState>,
    pub req_downstream: Requestor<(), DiscStockState>,
    pub req_environment: Requestor<(), BasicEnvironmentState>,
    pub withdraw_upstream: Requestor<(u32, EventId), Vec<ItemType>>,
    pub push_downstream: Output<(Vec<ItemType>, EventId)>,
    pub log_emitter: Output<ProcessLog>,

    // Configuration
    pub process_quantity_distr: Distribution,
    pub process_time_distr: Distribution,
    pub delay_modes: DelayModes,

    // Runtime state
    pub process_state: Option<(Duration, Vec<ItemType>)>,
    pub env_state: BasicEnvironmentState,

    // Internals
    pub time_to_next_process_event: Option<Duration>,
    pub time_to_next_delay_event: Option<Duration>,
    pub scheduled_event: Option<(MonotonicTime, ActionKey)>,
    pub next_event_index: u64,
    pub previous_check_time: MonotonicTime,
}

impl<
        ItemType: Clone + Debug + Serialize + Send + 'static,
        ProcessLog: Clone + Debug + Serialize + Send + 'static,
    > Default for DefaultDiscProcess<ItemType, ProcessLog>
{
    fn default() -> Self {
        DefaultDiscProcess {
            element_name: "DefaultDiscProcess".into(),
            element_code: "".into(),
            element_type: "DefaultDiscProcess".into(),

            req_upstream: Requestor::default(),
            req_downstream: Requestor::default(),
            req_environment: Requestor::default(),
            withdraw_upstream: Requestor::default(),
            push_downstream: Output::default(),
            log_emitter: Output::default(),

            process_quantity_distr: Distribution::default(),
            process_time_distr: Distribution::default(),
            delay_modes: DelayModes::default(),

            process_state: None,
            env_state: BasicEnvironmentState::Normal,

            time_to_next_process_event: None,
            time_to_next_delay_event: None,
            scheduled_event: None,
            next_event_index: 0,
            previous_check_time: MonotonicTime::EPOCH,
        }
    }
}

impl<ItemType: Clone + Debug + Serialize + Send + 'static> Model for DefaultDiscProcess<
    ItemType, 
    DiscProcessLog<DefaultDiscProcessLogType<ItemType>, ItemType>
>
where
    DiscProcessLog<DefaultDiscProcessLogType<ItemType>, ItemType>: Serialize,
    Self: ToLogRecord<
        DefaultDiscProcessLogType<ItemType>,
        DiscProcessLog<DefaultDiscProcessLogType<ItemType>, ItemType>
    > {
    fn init(
        mut self,
        ctx: &mut Context<Self>,
    ) -> impl Future<Output = InitializedModel<Self>> + Send {
        async move {
            let source_event_id = EventId(format!(
                "{}_{:06}",
                self.element_code, self.next_event_index
            ));
            self.update_state(source_event_id, ctx).await;
            self.into()
        }
    }
}

impl<ItemType> ToLogRecord<
        DefaultDiscProcessLogType<ItemType>,
        DiscProcessLog<DefaultDiscProcessLogType<ItemType>, ItemType>,
    > for DefaultDiscProcess<
        ItemType,
        DiscProcessLog<DefaultDiscProcessLogType<ItemType>, ItemType>,
    >
where
    ItemType: Clone + Debug + Serialize + Send + 'static,
{
    fn to_record(
        &mut self,
        now: MonotonicTime,
        source_event_id: EventId,
        event_id: EventId,
        details: DefaultDiscProcessLogType<ItemType>,
    ) -> DiscProcessLog<DefaultDiscProcessLogType<ItemType>, ItemType> {
        DiscProcessLog {
            time: now.to_chrono_date_time(0).unwrap().to_string(),
            event_id,
            source_event_id,
            element_name: self.element_name.clone(),
            element_type: self.element_type.clone(),
            details,
            phantom: std::marker::PhantomData,
        }
    }
}

impl<ItemType> DiscProcessCore<
        ItemType,
        DiscProcessLog<DefaultDiscProcessLogType<ItemType>, ItemType>,
        DefaultDiscProcessLogType<ItemType>,
    > for DefaultDiscProcess<
        ItemType,
        DiscProcessLog<DefaultDiscProcessLogType<ItemType>, ItemType>,
    >
where
    ItemType: Clone + Debug + Serialize + Send + 'static,
{
    fn element_name(&self) -> &str {
        &self.element_name
    }

    fn element_code(&self) -> &str {
        &self.element_code
    }

    fn element_type(&self) -> &str {
        &self.element_type
    }

    fn get_next_event_id(&mut self) -> EventId {
        let id = EventId(format!(
            "{}_{:06}",
            self.element_code, self.next_event_index
        ));
        self.next_event_index += 1;
        id
    }

    fn log_emitter(
        &mut self,
    ) -> &mut Output<
        DiscProcessLog<DefaultDiscProcessLogType<ItemType>, ItemType>,
    > {
        &mut self.log_emitter
    }

    fn scheduled_event(
        &mut self,
    ) -> &mut Option<(MonotonicTime, ActionKey)> {
        &mut self.scheduled_event
    }

    fn previous_check_time(&mut self) -> &mut MonotonicTime {
        &mut self.previous_check_time
    }

    fn delay_modes(&mut self) -> &mut DelayModes {
        &mut self.delay_modes
    }

    fn process_state(
        &mut self,
    ) -> &mut Option<(Duration, Vec<ItemType>)> {
        &mut self.process_state
    }

    fn env_state(&mut self) -> &mut BasicEnvironmentState {
        &mut self.env_state
    }

    fn req_environment(
        &mut self,
    ) -> &mut Requestor<(), BasicEnvironmentState> {
        &mut self.req_environment
    }

    fn time_to_next_process_event(
        &mut self,
    ) -> &mut Option<Duration> {
        &mut self.time_to_next_process_event
    }

    fn time_to_next_delay_event(
        &mut self,
    ) -> &mut Option<Duration> {
        &mut self.time_to_next_delay_event
    }

    fn log_type_withdraw_request(&self, quantity: u32) -> DefaultDiscProcessLogType<ItemType> {
        DefaultDiscProcessLogType::WithdrawRequest { quantity }
    }

    fn log_type_process_start(
        &self,
        quantity: u32,
        resources: Vec<ItemType>,
    ) -> DefaultDiscProcessLogType<ItemType> {
        DefaultDiscProcessLogType::ProcessStart { quantity, resources }
    }

    fn log_type_process_success(
        &self,
        quantity: u32,
        resources: Vec<ItemType>,
    ) -> DefaultDiscProcessLogType<ItemType> {
        DefaultDiscProcessLogType::ProcessSuccess { quantity, resources }
    }

    fn log_type_process_failure(
        &self,
        reason: &'static str,
    ) -> DefaultDiscProcessLogType<ItemType> {
        DefaultDiscProcessLogType::ProcessFailure { reason }
    }

    fn log_type_process_stopped(
        &self,
        reason: &'static str,
    ) -> DefaultDiscProcessLogType<ItemType> {
        DefaultDiscProcessLogType::ProcessStopped { reason }
    }

    fn log_type_process_continue(
        &self,
        reason: &'static str,
    ) -> DefaultDiscProcessLogType<ItemType> {
        DefaultDiscProcessLogType::ProcessContinue { reason }
    }

    fn log_type_delay_start(
        &self,
        delay_name: String,
    ) -> DefaultDiscProcessLogType<ItemType> {
        DefaultDiscProcessLogType::DelayStart { delay_name }
    }

    fn log_type_delay_end(
        &self,
        delay_name: String,
    ) -> DefaultDiscProcessLogType<ItemType> {
        DefaultDiscProcessLogType::DelayEnd { delay_name }
    }

    fn log_type_state_change(
        &self,
        new_state: DiscStockState,
    ) -> DefaultDiscProcessLogType<ItemType> {
        DefaultDiscProcessLogType::StateChange { new_state }
    }
}

impl<ItemType> DiscProcess<
        ItemType,
        DiscProcessLog<DefaultDiscProcessLogType<ItemType>, ItemType>,
        DefaultDiscProcessLogType<ItemType>,
    > for DefaultDiscProcess<
        ItemType,
        DiscProcessLog<DefaultDiscProcessLogType<ItemType>, ItemType>,
    >
where
    ItemType: Clone + Debug + Serialize + Send + 'static,
{
    fn req_upstream(&mut self) -> &mut Requestor<(), DiscStockState> {
        &mut self.req_upstream
    }

    fn withdraw_upstream(
        &mut self,
    ) -> &mut Requestor<(u32, EventId), Vec<ItemType>> {
        &mut self.withdraw_upstream
    }

    fn req_downstream(&mut self) -> &mut Requestor<(), DiscStockState> {
        &mut self.req_downstream
    }

    fn push_downstream(
        &mut self,
    ) -> &mut Output<(Vec<ItemType>, EventId)> {
        &mut self.push_downstream
    }

    fn process_quantity_distr(&mut self) -> &mut Distribution {
        &mut self.process_quantity_distr
    }

    fn process_time_distr(&mut self) -> &mut Distribution {
        &mut self.process_time_distr
    }
}