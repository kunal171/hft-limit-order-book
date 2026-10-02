//! FIFO queue of resting orders at one price.
//!
//! The queue is a doubly linked list threaded through the arena nodes, so an
//! order can be detached from any position without searching.

// Nothing uses this until the order book is switched over.
#![allow(dead_code)]

use super::arena::{OrderArena, Slot};
use crate::domain::Quantity;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(super) struct PriceLevel {
    /// Oldest order: first to match.
    pub head: Option<Slot>,
    /// Newest order: new arrivals go behind it.
    pub tail: Option<Slot>,
    pub total_quantity: Quantity,
}

impl PriceLevel {
    /// Append an order at the back of the queue.
    pub fn push_back(&mut self, arena: &mut OrderArena, slot: Slot) {
        let node = arena.get_mut(slot);
        node.prev = self.tail;
        node.next = None;
        self.total_quantity += node.order.remaining_qty;

        match self.tail {
            Some(tail) => arena.get_mut(tail).next = Some(slot),
            None => self.head = Some(slot),
        }
        self.tail = Some(slot);
    }

    /// Detach an order from any position and subtract its remaining quantity.
    pub fn unlink(&mut self, arena: &mut OrderArena, slot: Slot) {
        let node = arena.get(slot);
        let (prev, next) = (node.prev, node.next);
        self.total_quantity -= node.order.remaining_qty;

        match prev {
            Some(prev) => arena.get_mut(prev).next = next,
            None => self.head = next,
        }
        match next {
            Some(next) => arena.get_mut(next).prev = prev,
            None => self.tail = prev,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.head.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Order, OrderId, Side};

    /// Build a level holding orders 1, 2, 3 with quantity 5 each.
    fn level_of_three() -> (PriceLevel, OrderArena, [Slot; 3]) {
        let mut arena = OrderArena::default();
        let mut level = PriceLevel::default();
        let slots = [1, 2, 3].map(|id| {
            let slot = arena.insert(Order::new(id, Side::Buy, 100, 5));
            level.push_back(&mut arena, slot);
            slot
        });
        (level, arena, slots)
    }

    /// Walk the queue from head to tail.
    fn queue_ids(level: &PriceLevel, arena: &OrderArena) -> Vec<OrderId> {
        let mut ids = Vec::new();
        let mut cursor = level.head;
        while let Some(slot) = cursor {
            let node = arena.get(slot);
            ids.push(node.order.id);
            cursor = node.next;
        }
        ids
    }

    #[test]
    fn push_back_keeps_arrival_order() {
        let (level, arena, _) = level_of_three();

        assert_eq!(queue_ids(&level, &arena), vec![1, 2, 3]);
        assert_eq!(level.total_quantity, 15);
    }

    #[test]
    fn unlink_middle_joins_its_neighbours() {
        let (mut level, mut arena, slots) = level_of_three();

        level.unlink(&mut arena, slots[1]);

        assert_eq!(queue_ids(&level, &arena), vec![1, 3]);
        assert_eq!(level.total_quantity, 10);
    }

    #[test]
    fn unlink_head_and_tail_move_the_ends() {
        let (mut level, mut arena, slots) = level_of_three();

        level.unlink(&mut arena, slots[0]);
        level.unlink(&mut arena, slots[2]);

        assert_eq!(queue_ids(&level, &arena), vec![2]);
        assert_eq!(level.head, level.tail);
    }

    #[test]
    fn unlinking_every_order_empties_the_level() {
        let (mut level, mut arena, slots) = level_of_three();

        for slot in slots {
            level.unlink(&mut arena, slot);
        }

        assert!(level.is_empty());
        assert_eq!(level.tail, None);
        assert_eq!(level.total_quantity, 0);
    }
}
