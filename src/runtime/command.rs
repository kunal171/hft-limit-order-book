use serde::{Deserialize, Serialize};

use crate::{Order, OrderId, Price, Quantity};

/// Identifies one independently owned matching-engine shard.
pub type ShardId = u16;

/// Monotonically increasing command number within one shard.
pub type EngineSequence = u64;

/// Current serialized command format.
pub const ENGINE_SCHEMA_VERSION: u16 = 1;

/// A deterministic operation submitted to the matching engine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EngineCommand {
    Add(Order),

    Cancel {
        order_id: OrderId,
    },

    Modify {
        order_id: OrderId,
        new_price: Price,
        new_quantity: Quantity,
    },
}

/// Adds routing, ordering, and version information to a command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SequencedCommand {
    pub schema_version: u16,
    pub shard_id: ShardId,
    pub sequence: EngineSequence,
    pub command: EngineCommand,
}

impl SequencedCommand {
    pub fn new(shard_id: ShardId, sequence: EngineSequence, command: EngineCommand) -> Self {
        Self {
            schema_version: ENGINE_SCHEMA_VERSION,
            shard_id,
            sequence,
            command,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Side;

    #[test]
    fn sequenced_command_preserves_command_metadata() {
        let command = SequencedCommand::new(
            2,
            15,
            EngineCommand::Add(Order::new(100, Side::Buy, 50_000, 10)),
        );

        assert_eq!(command.schema_version, ENGINE_SCHEMA_VERSION);
        assert_eq!(command.shard_id, 2);
        assert_eq!(command.sequence, 15);
    }
}
