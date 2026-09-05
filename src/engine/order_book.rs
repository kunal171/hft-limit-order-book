use crate::Quantity;
use crate::domain::{BookEvent, BookSnapshot, Order, OrderId, Price, Side, Trade};
use crate::engine::config::{EventMode, OrderBookConfig};
use crate::error::OrderBookError;
use std::collections::{BTreeMap, HashMap, VecDeque};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OrderLocation {
    pub side: Side,
    pub price: Price,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PriceLevel {
    pub order_ids: VecDeque<OrderId>,
    pub total_quantity: Quantity,
}
/// A simple price-time priority limit order book.
///
/// `BTreeMap` keeps price levels sorted.
/// `VecDeque` preserves FIFO order inside each price level.
#[derive(Debug, Default)]
pub struct OrderBook {
    pub(super) bids: BTreeMap<Price, PriceLevel>,
    pub(super) asks: BTreeMap<Price, PriceLevel>,
    pub(super) orders: HashMap<OrderId, Order>,
    pub(super) order_locations: HashMap<OrderId, OrderLocation>,
    pub(super) events: Vec<BookEvent>,
    pub(super) config: OrderBookConfig,
}

impl OrderBook {
    /// Create an empty book.
    pub fn new() -> Self {
        Self::with_config(OrderBookConfig::default())
    }

    pub fn with_config(config: OrderBookConfig) -> Self {
        Self {
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            orders: HashMap::new(),
            order_locations: HashMap::new(),
            events: Vec::new(),
            config,
        }
    }

    /// Add an order and return all trades caused by that order.
    pub fn add_order(&mut self, order: Order) -> Result<Vec<Trade>, OrderBookError> {
        if order.remaining_qty == 0 {
            return Err(OrderBookError::ZeroQuantity);
        }
        if self.orders.contains_key(&order.id) {
            return Err(OrderBookError::DuplicateOrderId);
        }

        // Record accepted order only if config allows it.
        self.record_order_accepted(&order);

        let trades = match order.side {
            Side::Buy => self.match_buy_order(order),
            Side::Sell => self.match_sell_order(order),
        };

        for trade in &trades {
            self.record_trade_executed(&trade);
        }
        Ok(trades)
    }

    /// Highest resting buy price.
    pub fn best_bid(&self) -> Option<Price> {
        self.bids
            .iter()
            .rev()
            .find(|(_, level)| level.total_quantity > 0)
            .map(|(price, _)| *price)
    }

    pub fn best_ask(&self) -> Option<Price> {
        self.asks
            .iter()
            .find(|(_, level)| level.total_quantity > 0)
            .map(|(price, _)| *price)
    }

    /// Number of resting orders across both sides.
    pub fn resting_order_count(&self) -> usize {
        self.orders.len()
    }

    // Cancel the order
    pub fn cancel_order(&mut self, order_id: OrderId) -> Result<(), OrderBookError> {
        let location = self
            .order_locations
            .remove(&order_id)
            .ok_or(OrderBookError::UnknownOrderId)?;

        let order = self
            .orders
            .remove(&order_id)
            .ok_or(OrderBookError::UnknownOrderId)?;

        // Decrease quantity
        self.decrease_level_quantity(location, order.remaining_qty);
        //record cancel
        self.record_order_cancelled(order_id);

        Ok(())
    }

    fn remove_order_by_id(&mut self, order_id: OrderId) -> Option<Order> {
        let location = self.order_locations.remove(&order_id)?;
        let order = self.orders.remove(&order_id)?;

        self.decrease_level_quantity(location, order.remaining_qty);
        self.remove_order_id_from_level(location, order_id);

        Some(order)
    }

    pub fn modify_order(
        &mut self,
        order_id: OrderId,
        new_price: Price,
        new_quantity: Quantity,
    ) -> Result<Vec<Trade>, OrderBookError> {
        if new_quantity == 0 {
            return Err(OrderBookError::ZeroQuantity);
        }
        let old_order = self
            .remove_order_by_id(order_id)
            .ok_or(OrderBookError::UnknownOrderId)?;

        let updated_order = Order::new(old_order.id, old_order.side, new_price, new_quantity);

        let trades = match updated_order.side {
            Side::Buy => self.match_buy_order(updated_order),
            Side::Sell => self.match_sell_order(updated_order),
        };

        self.record_order_modified(order_id, new_price, new_quantity);

        for trade in &trades {
            self.record_trade_executed(trade);
        }

        Ok(trades)
    }

    /// Return all events emitted by this order book.
    pub fn events(&self) -> &[BookEvent] {
        &self.events
    }

    /// Return a full snapshot of the current book state.
    ///
    /// Bids are returned from highest price to lowest price.
    /// Asks are returned from lowest price to highest price.
    pub fn snapshot(&self) -> BookSnapshot {
        let bids = self
            .bids
            .iter()
            .rev()
            .filter_map(|(price, order_ids)| {
                let orders: Vec<Order> = order_ids
                    .order_ids
                    .iter()
                    .filter_map(|id| self.orders.get(id).cloned())
                    .collect();
                if orders.is_empty() {
                    None
                } else {
                    Some((*price, orders))
                }
            })
            .collect();

        let asks = self
            .asks
            .iter()
            .filter_map(|(price, order_ids)| {
                let orders: Vec<Order> = order_ids
                    .order_ids
                    .iter()
                    .filter_map(|id| self.orders.get(id).cloned())
                    .collect();
                if orders.is_empty() {
                    None
                } else {
                    Some((*price, orders))
                }
            })
            .collect();

        BookSnapshot { bids, asks }
    }

    pub fn rest_order(&mut self, order: Order) {
        let order_id = order.id;
        let side = order.side;
        let price = order.price;

        let levels = match side {
            Side::Buy => &mut self.bids,
            Side::Sell => &mut self.asks,
        };

        let level = levels.entry(price).or_insert_with(|| PriceLevel {
            order_ids: VecDeque::new(),
            total_quantity: 0,
        });

        level.order_ids.push_back(order_id);
        level.total_quantity += order.remaining_qty;

        self.order_locations
            .insert(order_id, OrderLocation { side, price });

        self.orders.insert(order_id, order);
    }

    fn remove_order_id_from_level(&mut self, location: OrderLocation, order_id: OrderId) {
        let levels = match location.side {
            Side::Buy => &mut self.bids,
            Side::Sell => &mut self.asks,
        };

        if let Some(level) = levels.get_mut(&location.price) {
            if let Some(index) = level.order_ids.iter().position(|id| *id == order_id) {
                level.order_ids.remove(index);
            }

            if level.order_ids.is_empty() {
                levels.remove(&location.price);
            }
        }
    }

    fn decrease_level_quantity(&mut self, location: OrderLocation, quantity: Quantity) {
        let levels = match location.side {
            Side::Buy => &mut self.bids,
            Side::Sell => &mut self.asks,
        };

        let should_remove_level = if let Some(level) = levels.get_mut(&location.price) {
            debug_assert!(level.total_quantity >= quantity);
            level.total_quantity -= quantity;
            level.total_quantity == 0
        } else {
            false
        };

        if should_remove_level {
            levels.remove(&location.price);
        }
    }

    fn record_order_accepted(&mut self, order: &Order) {
        if self.config.event_mode == EventMode::Full {
            self.events.push(BookEvent::OrderAccepted {
                order: order.clone(),
            });
        }
    }

    fn record_trade_executed(&mut self, trade: &Trade) {
        match self.config.event_mode {
            EventMode::Full | EventMode::TradesOnly => {
                self.events.push(BookEvent::TradeExecuted {
                    trade: trade.clone(),
                });
            }
            EventMode::Disabled => {}
        }
    }

    fn record_order_cancelled(&mut self, order_id: OrderId) {
        if self.config.event_mode == EventMode::Full {
            self.events.push(BookEvent::OrderCancelled { order_id });
        }
    }
    fn record_order_modified(
        &mut self,
        order_id: OrderId,
        new_price: Price,
        new_quantity: Quantity,
    ) {
        if self.config.event_mode == EventMode::Full {
            self.events.push(BookEvent::OrderModified {
                order_id,
                new_price,
                new_quantity,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Side;
    use crate::metrics::calculate_book_metrics;

    #[test]
    fn buy_order_rests_when_there_is_no_matching_ask() {
        let mut book = OrderBook::new();

        let trades = book
            .add_order(Order::new(1, Side::Buy, 100, 10))
            .expect("valid buy order should be accepted");

        assert!(trades.is_empty());
        assert_eq!(book.best_bid(), Some(100));
        assert_eq!(book.best_ask(), None);
        assert_eq!(book.resting_order_count(), 1);
    }

    #[test]
    fn sell_order_matches_existing_bid() {
        let mut book = OrderBook::new();
        book.add_order(Order::new(1, Side::Buy, 100, 10))
            .expect("valid buy order should be accepted");

        let trades = book
            .add_order(Order::new(2, Side::Sell, 99, 4))
            .expect("valid sell order should be accepted");

        assert_eq!(trades, vec![Trade::new(1, 2, 100, 4)]);
        assert_eq!(book.best_bid(), Some(100));
        assert_eq!(book.resting_order_count(), 1);
    }

    #[test]
    fn orders_at_same_price_match_fifo() {
        let mut book = OrderBook::new();
        book.add_order(Order::new(1, Side::Buy, 100, 5))
            .expect("first valid buy order should be accepted");
        book.add_order(Order::new(2, Side::Buy, 100, 5))
            .expect("second valid buy order should be accepted");

        let trades = book
            .add_order(Order::new(3, Side::Sell, 100, 8))
            .expect("valid sell order should be accepted");

        assert_eq!(
            trades,
            vec![Trade::new(1, 3, 100, 5), Trade::new(2, 3, 100, 3)]
        );
        assert_eq!(book.best_bid(), Some(100));
        assert_eq!(book.resting_order_count(), 1);
    }

    #[test]
    fn better_price_matches_before_older_worse_price() {
        let mut book = OrderBook::new();
        book.add_order(Order::new(1, Side::Buy, 99, 5))
            .expect("valid buy order should be accepted");
        book.add_order(Order::new(2, Side::Buy, 101, 5))
            .expect("better-priced valid buy order should be accepted");

        let trades = book
            .add_order(Order::new(3, Side::Sell, 99, 5))
            .expect("valid sell order should be accepted");

        assert_eq!(trades, vec![Trade::new(2, 3, 101, 5)]);
        assert_eq!(book.best_bid(), Some(99));
    }

    #[test]
    fn sell_order_rests_when_there_is_no_matching_bid() {
        let mut book = OrderBook::new();

        let trades = book
            .add_order(Order::new(1, Side::Sell, 101, 10))
            .expect("valid sell order should be accepted");

        assert!(trades.is_empty());
        assert_eq!(book.best_bid(), None);
        assert_eq!(book.best_ask(), Some(101));
        assert_eq!(book.resting_order_count(), 1);
    }

    #[test]
    fn sell_does_not_match_bid_below_limit() {
        let mut book = OrderBook::new();
        book.add_order(Order::new(1, Side::Buy, 100, 10))
            .expect("valid buy order should be accepted");

        let trades = book
            .add_order(Order::new(2, Side::Sell, 101, 5))
            .expect("valid sell order should be accepted");

        assert!(trades.is_empty());
        assert_eq!(book.best_bid(), Some(100));
        assert_eq!(book.best_ask(), Some(101));
        assert_eq!(book.resting_order_count(), 2);
    }

    #[test]
    fn buy_does_not_match_ask_above_limit() {
        let mut book = OrderBook::new();
        book.add_order(Order::new(1, Side::Sell, 100, 10))
            .expect("valid sell order should be accepted");

        let trades = book
            .add_order(Order::new(2, Side::Buy, 99, 5))
            .expect("valid buy order should be accepted");

        assert!(trades.is_empty());
        assert_eq!(book.best_bid(), Some(99));
        assert_eq!(book.best_ask(), Some(100));
        assert_eq!(book.resting_order_count(), 2);
    }

    #[test]
    fn buy_sweeps_multiple_ask_levels() {
        let mut book = OrderBook::new();
        book.add_order(Order::new(1, Side::Sell, 100, 5))
            .expect("valid sell order should be accepted");
        book.add_order(Order::new(2, Side::Sell, 101, 5))
            .expect("valid sell order should be accepted");
        book.add_order(Order::new(3, Side::Sell, 102, 5))
            .expect("valid sell order should be accepted");

        let trades = book
            .add_order(Order::new(4, Side::Buy, 102, 12))
            .expect("valid buy order should be accepted");

        assert_eq!(
            trades,
            vec![
                Trade::new(1, 4, 100, 5),
                Trade::new(2, 4, 101, 5),
                Trade::new(3, 4, 102, 2),
            ]
        );
        assert_eq!(book.best_ask(), Some(102));
        assert_eq!(book.resting_order_count(), 1);
    }

    #[test]
    fn sell_sweeps_multiple_bid_levels() {
        let mut book = OrderBook::new();
        book.add_order(Order::new(1, Side::Buy, 100, 5))
            .expect("valid buy order should be accepted");
        book.add_order(Order::new(2, Side::Buy, 101, 5))
            .expect("valid buy order should be accepted");
        book.add_order(Order::new(3, Side::Buy, 102, 5))
            .expect("valid buy order should be accepted");

        let trades = book
            .add_order(Order::new(4, Side::Sell, 99, 12))
            .expect("valid sell order should be accepted");

        assert_eq!(
            trades,
            vec![
                Trade::new(3, 4, 102, 5),
                Trade::new(2, 4, 101, 5),
                Trade::new(1, 4, 100, 2),
            ]
        );
        assert_eq!(book.best_bid(), Some(100));
        assert_eq!(book.resting_order_count(), 1);
    }

    #[test]
    fn rejects_zero_quantity_order() {
        let mut book = OrderBook::new();

        let result = book.add_order(Order::new(1, Side::Buy, 100, 0));

        assert_eq!(result, Err(OrderBookError::ZeroQuantity));
        assert_eq!(book.resting_order_count(), 0);
    }

    #[test]
    fn rejects_duplicate_resting_order_id() {
        let mut book = OrderBook::new();

        book.add_order(Order::new(1, Side::Buy, 100, 10))
            .expect("first order should be accepted");

        let result = book.add_order(Order::new(1, Side::Sell, 101, 5));

        assert_eq!(result, Err(OrderBookError::DuplicateOrderId));
        assert_eq!(book.resting_order_count(), 1);
    }

    #[test]
    fn cancel_resting_bid_order() {
        let mut book = OrderBook::new();

        book.add_order(Order::new(1, Side::Buy, 100, 10))
            .expect("valid order should be accepted");

        let result = book.cancel_order(1);

        assert_eq!(result, Ok(()));
        assert_eq!(book.best_bid(), None);
        assert_eq!(book.resting_order_count(), 0);
    }

    #[test]
    fn cancel_resting_ask_order() {
        let mut book = OrderBook::new();

        book.add_order(Order::new(1, Side::Sell, 101, 10))
            .expect("valid order should be accepted");

        let result = book.cancel_order(1);

        assert_eq!(result, Ok(()));
        assert_eq!(book.best_ask(), None);
        assert_eq!(book.resting_order_count(), 0);
    }

    #[test]
    fn cancel_unknown_order_returns_error() {
        let mut book = OrderBook::new();

        let result = book.cancel_order(999);

        assert_eq!(result, Err(OrderBookError::UnknownOrderId));
    }

    #[test]
    fn cancel_one_order_keeps_price_level_when_other_orders_remain() {
        let mut book = OrderBook::new();

        book.add_order(Order::new(1, Side::Buy, 100, 10))
            .expect("first order should be accepted");
        book.add_order(Order::new(2, Side::Buy, 100, 5))
            .expect("second order should be accepted");

        let result = book.cancel_order(1);

        assert_eq!(result, Ok(()));
        assert_eq!(book.best_bid(), Some(100));
        assert_eq!(book.resting_order_count(), 1);
    }

    #[test]
    fn cancelled_order_does_not_count_as_liquidity() {
        let mut book = OrderBook::new();

        book.add_order(Order::new(1, Side::Buy, 100, 10))
            .expect("valid bid should be accepted");
        book.cancel_order(1).expect("cancel should succeed");

        let snapshot = book.snapshot();
        let metrics = calculate_book_metrics(&snapshot);

        assert_eq!(book.best_bid(), None);
        assert_eq!(book.resting_order_count(), 0);
        assert!(snapshot.bids.is_empty());
        assert_eq!(metrics.total_bid_quantity, 0);
        assert_eq!(metrics.imbalance, None);
    }

    #[test]
    fn modify_unknown_order_returns_error() {
        let mut book = OrderBook::new();

        let result = book.modify_order(999, 100, 10);

        assert_eq!(result, Err(OrderBookError::UnknownOrderId));
    }

    #[test]
    fn modify_order_rejects_zero_quantity() {
        let mut book = OrderBook::new();

        book.add_order(Order::new(1, Side::Buy, 100, 10))
            .expect("valid order should be accepted");

        let result = book.modify_order(1, 100, 0);

        assert_eq!(result, Err(OrderBookError::ZeroQuantity));
    }

    #[test]
    fn modify_zero_quantity_does_not_remove_existing_order() {
        let mut book = OrderBook::new();

        book.add_order(Order::new(1, Side::Buy, 100, 10))
            .expect("valid order should be accepted");

        let result = book.modify_order(1, 100, 0);

        assert_eq!(result, Err(OrderBookError::ZeroQuantity));
        assert_eq!(book.best_bid(), Some(100));
        assert_eq!(book.resting_order_count(), 1);
    }

    #[test]
    fn modify_resting_bid_price() {
        let mut book = OrderBook::new();

        book.add_order(Order::new(1, Side::Buy, 100, 10))
            .expect("valid order should be accepted");

        let trades = book
            .modify_order(1, 105, 10)
            .expect("modify should succeed");

        assert!(trades.is_empty());
        assert_eq!(book.best_bid(), Some(105));
        assert_eq!(book.resting_order_count(), 1);
    }

    #[test]
    fn partial_fill_reduces_depth_but_keeps_remaining_liquidity() {
        let mut book = OrderBook::new();

        book.add_order(Order::new(1, Side::Buy, 100, 10))
            .expect("valid bid should be accepted");

        let trades = book
            .add_order(Order::new(2, Side::Sell, 100, 4))
            .expect("crossing sell should trade");

        let snapshot = book.snapshot();
        let metrics = calculate_book_metrics(&snapshot);

        assert_eq!(trades, vec![Trade::new(1, 2, 100, 4)]);
        assert_eq!(book.best_bid(), Some(100));
        assert_eq!(book.resting_order_count(), 1);
        assert_eq!(snapshot.bids[0].1[0].remaining_qty, 6);
        assert_eq!(metrics.total_bid_quantity, 6);
        assert_eq!(metrics.total_ask_quantity, 0);
        assert_eq!(metrics.imbalance, Some(1.0));
    }

    #[test]
    fn add_order_records_order_accepted_event() {
        let mut book = OrderBook::new();

        book.add_order(Order::new(1, Side::Buy, 100, 10))
            .expect("valid order should be accepted");

        assert_eq!(
            book.events(),
            &[BookEvent::OrderAccepted {
                order: Order::new(1, Side::Buy, 100, 10),
            }]
        );
    }

    #[test]
    fn matching_order_records_trade_event() {
        let mut book = OrderBook::new();

        book.add_order(Order::new(1, Side::Buy, 100, 10))
            .expect("resting buy order should be accepted");
        let trades = book
            .add_order(Order::new(2, Side::Sell, 99, 4))
            .expect("crossing sell order should be accepted");

        assert_eq!(trades, vec![Trade::new(1, 2, 100, 4)]);
        assert_eq!(
            book.events(),
            &[
                BookEvent::OrderAccepted {
                    order: Order::new(1, Side::Buy, 100, 10),
                },
                BookEvent::OrderAccepted {
                    order: Order::new(2, Side::Sell, 99, 4),
                },
                BookEvent::TradeExecuted {
                    trade: Trade::new(1, 2, 100, 4),
                },
            ]
        );
    }

    #[test]
    fn cancel_order_records_cancelled_event() {
        let mut book = OrderBook::new();

        book.add_order(Order::new(1, Side::Buy, 100, 10))
            .expect("valid order should be accepted");
        book.cancel_order(1).expect("cancel should succeed");

        assert_eq!(
            book.events(),
            &[
                BookEvent::OrderAccepted {
                    order: Order::new(1, Side::Buy, 100, 10),
                },
                BookEvent::OrderCancelled { order_id: 1 },
            ]
        );
    }

    #[test]
    fn modify_order_records_modified_event() {
        let mut book = OrderBook::new();

        book.add_order(Order::new(1, Side::Buy, 100, 10))
            .expect("valid order should be accepted");
        book.modify_order(1, 105, 7).expect("modify should succeed");

        assert_eq!(
            book.events(),
            &[
                BookEvent::OrderAccepted {
                    order: Order::new(1, Side::Buy, 100, 10),
                },
                BookEvent::OrderModified {
                    order_id: 1,
                    new_price: 105,
                    new_quantity: 7,
                },
            ]
        );
    }

    #[test]
    fn snapshot_returns_bids_high_to_low_and_asks_low_to_high() {
        let mut book = OrderBook::new();

        book.add_order(Order::new(1, Side::Buy, 100, 5)).unwrap();
        book.add_order(Order::new(2, Side::Buy, 102, 5)).unwrap();
        book.add_order(Order::new(3, Side::Sell, 105, 5)).unwrap();
        book.add_order(Order::new(4, Side::Sell, 103, 5)).unwrap();

        let snapshot = book.snapshot();

        assert_eq!(snapshot.bids[0].0, 102);
        assert_eq!(snapshot.bids[1].0, 100);
        assert_eq!(snapshot.asks[0].0, 103);
        assert_eq!(snapshot.asks[1].0, 105);
    }

    #[test]
    fn disabled_event_mode_records_no_events() {
        let mut book = OrderBook::with_config(OrderBookConfig {
            event_mode: EventMode::Disabled,
        });

        book.add_order(Order::new(1, Side::Buy, 100, 10))
            .expect("valid order should be accepted");

        book.cancel_order(1).expect("cancel should succeed");

        assert!(book.events().is_empty());
    }

    #[test]
    fn trades_only_event_mode_records_only_trades() {
        let mut book = OrderBook::with_config(OrderBookConfig {
            event_mode: EventMode::TradesOnly,
        });
        book.add_order(Order::new(1, Side::Buy, 100, 10))
            .expect("resting order should be accepted");

        book.add_order(Order::new(2, Side::Sell, 100, 4))
            .expect("crossing order should be accepted");

        assert_eq!(
            book.events(),
            &[BookEvent::TradeExecuted {
                trade: Trade::new(1, 2, 100, 4),
            }]
        );
    }

    #[test]
    fn full_event_mode_records_all_events() {
        let mut book = OrderBook::with_config(OrderBookConfig {
            event_mode: EventMode::Full,
        });

        book.add_order(Order::new(1, Side::Buy, 100, 10))
            .expect("valid order should be accepted");

        book.cancel_order(1).expect("cancel should succeed");

        assert_eq!(
            book.events(),
            &[
                BookEvent::OrderAccepted {
                    order: Order::new(1, Side::Buy, 100, 10),
                },
                BookEvent::OrderCancelled { order_id: 1 },
            ]
        );
    }
}
