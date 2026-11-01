use serde::{Deserialize, Serialize};

use crate::{BookEvent, OrderBookError};

use super::command::{ENGINE_SCHEMA_VERSION, EngineSequence, ShardId};

/// The outcome of processing a command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EngineEvent {
    Book(BookEvent),
    Rejected { reason: OrderBookError },
}

/// Identifies one event produced by one command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SequencedEvent {
    pub schema_version: u16,
    pub shard_id: ShardId,
    pub sequence: EngineSequence,
    pub event_index: u32,
    pub event: EngineEvent,
}

impl SequencedEvent {
    pub fn new(
        shard_id: ShardId,
        sequence: EngineSequence,
        event_index: u32,
        event: EngineEvent,
    ) -> Self {
        Self {
            schema_version: ENGINE_SCHEMA_VERSION,
            shard_id,
            sequence,
            event_index,
            event,
        }
    }
}
