use crate::{BookEvent, OrderBook, OrderBookError};

/// Rebuild an order book by applying historical events again.
pub fn replay_events(events: &[BookEvent]) -> Result<OrderBook, OrderBookError> {
    let mut book = OrderBook::new();

    for event in events {
        match event {
            BookEvent::OrderAccepted { order } => {
                book.add_order(order.clone())?;
            }

            BookEvent::OrderCancelled { order_id } => {
                book.cancel_order(*order_id)?;
            }

            BookEvent::OrderModified {
                order_id,
                new_price,
                new_quantity,
            } => {
                book.modify_order(*order_id, *new_price, *new_quantity)?;
            }

            BookEvent::TradeExecuted { .. } => {
                // Do nothing.
                // Trades are produced automatically when we replay OrderAccepted/OrderModified.
            }
        }
    }

    Ok(book)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Order, OrderBook, Side};

    #[test]
    fn replay_rebuilds_same_book_state() {
        let mut original = OrderBook::new();

        original
            .add_order(Order::new(1, Side::Buy, 100, 10))
            .expect("order should be accepted");

        original
            .add_order(Order::new(2, Side::Sell, 99, 4))
            .expect("order should be accepted");

        let replayed = replay_events(original.events()).expect("replay should succeed");

        assert_eq!(replayed.snapshot(), original.snapshot());
        assert_eq!(replayed.events(), original.events());
    }

    // Replay rebuilds with canceled orders
    #[test]
    fn replay_rebuilds_book_after_cancel() {
        let mut original = OrderBook::new();

        original
            .add_order(Order::new(1, Side::Buy, 100, 10))
            .expect("order should be accepted");

        original
            .add_order(Order::new(2, Side::Buy, 101, 5))
            .expect("order should be accepted");

        original.cancel_order(1).expect("cancel should succeed");

        let replayed = replay_events(original.events()).expect("replay should succeed");

        assert_eq!(replayed.snapshot(), original.snapshot());
        assert_eq!(replayed.events(), original.events());
    }

    //Relay builds with modified orders
    #[test]
    fn replay_rebuilds_book_after_modify() {
        let mut original = OrderBook::new();

        original
            .add_order(Order::new(1, Side::Buy, 100, 10))
            .expect("order should be accepted");

        original
            .modify_order(1, 105, 7)
            .expect("modify should succeed");

        let replayed = replay_events(original.events()).expect("replay should succeed");

        assert_eq!(replayed.snapshot(), original.snapshot());
        assert_eq!(replayed.events(), original.events());
    }

    #[test]
    fn replay_rebuilds_book_after_multi_level_matching() {
        let mut original = OrderBook::new();

        original
            .add_order(Order::new(1, Side::Sell, 100, 5))
            .expect("order should be accepted");

        original
            .add_order(Order::new(2, Side::Sell, 101, 5))
            .expect("order should be accepted");

        original
            .add_order(Order::new(3, Side::Sell, 102, 5))
            .expect("order should be accepted");

        original
            .add_order(Order::new(4, Side::Buy, 102, 12))
            .expect("order should be accepted");

        let replayed = replay_events(original.events()).expect("replay should succeed");

        assert_eq!(replayed.snapshot(), original.snapshot());
        assert_eq!(replayed.events(), original.events());
    }
}
