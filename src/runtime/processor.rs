use crate::OrderBook;
use super::{EngineSequence, ShardId};

pub struct EngineProcessor {
    shard_id: ShardId,
    next_sequence: EngineSequence,
    book: OrderBook,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessorError {
    UnsupportedSchema,
    WrongShard,
    WrongSequence { expected: EngineSequence, received: EngineSequence },
    SequenceExhausted,
}

impl EngineProcessor {
    pub fn new(shard_id: ShardId) -> Self {
        Self {
            shard_id,
            next_sequence: 1,
            book: OrderBook::new(),
        }
    }
    pub fn process(
        &mut self,
        input: SequencedCommand,
    ) -> Result<Vec<SequencedEvent>, ProcessorError> {
        if input.schema_version != ENGINE_SCHEMA_VERSION {
            return Err(ProcessorError::UnsupportedSchema);
        }
        if input.shard_id != self.shard_id {
            return Err(ProcessorError::WrongShard);
        }
        if input.sequence != self.next_sequence {
            return Err(ProcessorError::WrongSequence {
                expected: self.next_sequence,
                received: input.sequence,
            });
        }

        // Check overflow before applying a command; no partial state change.
        let next_sequence = self
            .next_sequence
            .checked_add(1)
            .ok_or(ProcessorError::SequenceExhausted)?;

        let outcome = match input.command {
            EngineCommand::Add(order) => self.book.add_order(order).map(|_| ()),
            EngineCommand::Cancel { order_id } => self.book.cancel_order(order_id),
            EngineCommand::Modify {
                order_id,
                new_price,
                new_quantity,
            } => self
                .book
                .modify_order(order_id, new_price, new_quantity)
                .map(|_| ()),
        };

        // Even a book rejection occupies one position in the command stream.
        self.next_sequence = next_sequence;

        match outcome {
            Ok(()) => Ok(self
                .book
                .drain_events()
                .enumerate()
                .map(|(index, event)| {
                    SequencedEvent::new(
                        self.shard_id,
                        input.sequence,
                        u32::try_from(index).expect("too many events from one command"),
                        EngineEvent::Book(event),
                    )
                })
                .collect()),
            Err(reason) => Ok(vec![SequencedEvent::new(
                self.shard_id,
                input.sequence,
                0,
                EngineEvent::Rejected { reason },
            )]),
        }
    }
}