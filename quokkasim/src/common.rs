use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
/// A short, lightweight identifier for an event. Very useful for understanding causal flow of events via log files.
/// Conventionally of the form `PROC_123456`, with a prefix uniquely identifying the process, and suffix an auto-incrementing number.
pub struct EventId(pub String);

impl EventId {
    pub fn from_init() -> EventId {
        EventId("INIT_000000".to_string())
    }

    pub fn from_scheduler() -> EventId {
        EventId("SCH_000000".to_string())
    }
}

pub trait ToLogRecord<DetailsType, LogType> {
    fn to_record(&mut self, source_event_id: EventId, event_id: EventId, details: DetailsType) -> LogType;
}

pub trait StockState {
    fn is_same_state(&self, other: &Self) -> bool;
}