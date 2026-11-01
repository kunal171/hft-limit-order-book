use crate::OrderBook;
use super::{
    ENGINE_SCHEMA_VERSION, EngineCommand, EngineEvent, EngineSequence,
    SequencedCommand, SequencedEvent, ShardId,
};

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


#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Order, OrderBookError, Side};

    #[test]
    fn rejected_order_consumes_its_sequence() {
        let mut processor = EngineProcessor::new(2);

        // The command is correctly sequenced, but its quantity is invalid.
        let rejected = processor
            .process(SequencedCommand::new(
                2,
                1,
                EngineCommand::Add(Order::new(10, Side::Buy, 100, 0)),
            ))
            .unwrap();

        assert_eq!(rejected.len(), 1);
        assert_eq!(
            rejected[0].event,
            EngineEvent::Rejected {
                reason: OrderBookError::ZeroQuantity,
            }
        );

        // Rejection used sequence 1, so the next valid command uses 2.
        let accepted = processor
            .process(SequencedCommand::new(
                2,
                2,
                EngineCommand::Add(Order::new(11, Side::Buy, 100, 5)),
            ))
            .unwrap();

        assert_eq!(accepted.len(), 1);
        assert_eq!(accepted[0].sequence, 2);
        assert_eq!(accepted[0].event_index, 0);
    }
}