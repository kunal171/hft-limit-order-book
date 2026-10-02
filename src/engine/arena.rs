//! Slab storage for resting orders.
//!
//! Orders live in one `Vec` and are referred to by index. Freed slots are
//! reused, so a warmed-up book does not allocate.

// Nothing uses the arena until the order book is switched over.
#![allow(dead_code)]

use crate::domain::Order;

/// Position of a resting order inside the arena.
pub(super) type Slot = u32;

/// A resting order plus its neighbours in the price-level queue.
#[derive(Debug)]
pub(super) struct OrderNode {
    pub order: Order,
    pub prev: Option<Slot>,
    pub next: Option<Slot>,
}

#[derive(Debug)]
enum Entry {
    Occupied(OrderNode),
    /// Free slot that points at the next free slot.
    Vacant {
        next_free: Option<Slot>,
    },
}

#[derive(Debug, Default)]
pub(super) struct OrderArena {
    entries: Vec<Entry>,
    free_head: Option<Slot>,
    len: usize,
}

impl OrderArena {
    /// Store an order and return its slot. Reuses a free slot when one exists.
    pub fn insert(&mut self, order: Order) -> Slot {
        let node = OrderNode {
            order,
            prev: None,
            next: None,
        };
        self.len += 1;

        let Some(slot) = self.free_head else {
            let slot = self.entries.len() as Slot;
            self.entries.push(Entry::Occupied(node));
            return slot;
        };

        self.free_head = match &self.entries[slot as usize] {
            Entry::Vacant { next_free } => *next_free,
            Entry::Occupied(_) => unreachable!("free list points at an occupied slot"),
        };
        self.entries[slot as usize] = Entry::Occupied(node);
        slot
    }

    /// Take an order out and put its slot on the free list.
    pub fn remove(&mut self, slot: Slot) -> Order {
        let entry = std::mem::replace(
            &mut self.entries[slot as usize],
            Entry::Vacant {
                next_free: self.free_head,
            },
        );

        let Entry::Occupied(node) = entry else {
            panic!("removed a vacant slot");
        };

        self.free_head = Some(slot);
        self.len -= 1;
        node.order
    }

    pub fn get(&self, slot: Slot) -> &OrderNode {
        match &self.entries[slot as usize] {
            Entry::Occupied(node) => node,
            Entry::Vacant { .. } => panic!("read a vacant slot"),
        }
    }

    pub fn get_mut(&mut self, slot: Slot) -> &mut OrderNode {
        match &mut self.entries[slot as usize] {
            Entry::Occupied(node) => node,
            Entry::Vacant { .. } => panic!("read a vacant slot"),
        }
    }

    /// Number of orders currently stored.
    pub fn len(&self) -> usize {
        self.len
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Side;

    #[test]
    fn insert_then_get_returns_the_order() {
        let mut arena = OrderArena::default();

        let slot = arena.insert(Order::new(1, Side::Buy, 100, 5));

        assert_eq!(arena.get(slot).order.id, 1);
        assert_eq!(arena.len(), 1);
    }

    #[test]
    fn removed_slot_is_reused() {
        let mut arena = OrderArena::default();
        let first = arena.insert(Order::new(1, Side::Buy, 100, 5));
        arena.insert(Order::new(2, Side::Buy, 100, 5));

        let removed = arena.remove(first);
        let reused = arena.insert(Order::new(3, Side::Buy, 100, 5));

        assert_eq!(removed.id, 1);
        assert_eq!(reused, first);
        assert_eq!(arena.len(), 2);
    }

    #[test]
    fn free_slots_are_reused_most_recent_first() {
        let mut arena = OrderArena::default();
        let a = arena.insert(Order::new(1, Side::Buy, 100, 5));
        let b = arena.insert(Order::new(2, Side::Buy, 100, 5));
        arena.remove(a);
        arena.remove(b);

        assert_eq!(arena.insert(Order::new(3, Side::Buy, 100, 5)), b);
        assert_eq!(arena.insert(Order::new(4, Side::Buy, 100, 5)), a);
    }
}
