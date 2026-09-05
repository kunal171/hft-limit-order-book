use crate::domain::{Order, Price, Trade};

use super::order_book::OrderBook;

impl OrderBook {
    /// Match an incoming buy order against the cheapest asks first.
    pub(super) fn match_buy_order(&mut self, mut incoming: Order) -> Vec<Trade> {
        let mut trades = Vec::new();

        while !incoming.is_filled() {
            let Some(best_ask_price) = self.best_ask() else {
                break;
            };

            // A buy can only trade with asks priced at or below its limit price.
            if best_ask_price > incoming.price {
                break;
            }

            self.match_at_ask_level(best_ask_price, &mut incoming, &mut trades);
        }

        // Any unfilled quantity becomes a resting bid.
        if !incoming.is_filled() {
            // Only resting orders belong in the active-order index.
            self.rest_order(incoming);
        }

        trades
    }

    /// Match an incoming sell order against the most expensive bids first.
    pub(super) fn match_sell_order(&mut self, mut incoming: Order) -> Vec<Trade> {
        let mut trades = Vec::new();

        while !incoming.is_filled() {
            let Some(best_bid_price) = self.best_bid() else {
                break;
            };

            // A sell can only trade with bids priced at or above its limit price.
            if best_bid_price < incoming.price {
                break;
            }

            self.match_at_bid_level(best_bid_price, &mut incoming, &mut trades);
        }

        // Any unfilled quantity becomes a resting ask.
        if !incoming.is_filled() {
            // Only resting orders belong in the active-order index.
            self.rest_order(incoming);
        }

        trades
    }

    /// Match against one ask price level.
    fn match_at_ask_level(&mut self, price: Price, incoming: &mut Order, trades: &mut Vec<Trade>) {
        let mut should_remove_level = false;

        if let Some(level) = self.asks.get_mut(&price) {
            while !incoming.is_filled() {
                let Some(resting_id) = level.order_ids.pop_front() else {
                    break;
                };

                let Some(mut resting) = self.orders.remove(&resting_id) else {
                    continue; // stale cancelled id
                };

                let traded_qty = incoming.remaining_qty.min(resting.remaining_qty);
                incoming.remaining_qty -= traded_qty;
                resting.remaining_qty -= traded_qty;

                level.total_quantity -= traded_qty;

                trades.push(Trade::new(resting.id, incoming.id, price, traded_qty));

                // A fully filled resting order must leave the active-order index.
                // A partially filled resting order keeps its FIFO position.
                if resting.is_filled() {
                    self.order_locations.remove(&resting.id);
                } else {
                    self.orders.insert(resting.id, resting);
                    level.order_ids.push_front(resting_id);
                    break;
                }
            }

            should_remove_level = level.order_ids.is_empty() || level.total_quantity == 0;
        }

        if should_remove_level {
            self.asks.remove(&price);
        }
    }

    /// Match against one bid price level.
    fn match_at_bid_level(&mut self, price: Price, incoming: &mut Order, trades: &mut Vec<Trade>) {
        let mut should_remove_level = false;

        if let Some(level) = self.bids.get_mut(&price) {
            while !incoming.is_filled() {
                let Some(resting_id) = level.order_ids.pop_front() else {
                    break;
                };

                let Some(mut resting) = self.orders.remove(&resting_id) else {
                    continue; // stale cancelled id
                };
                let traded_qty = incoming.remaining_qty.min(resting.remaining_qty);
                incoming.remaining_qty -= traded_qty;
                resting.remaining_qty -= traded_qty;

                //update total quantity
                level.total_quantity -= traded_qty;

                trades.push(Trade::new(resting.id, incoming.id, price, traded_qty));

                // A fully filled resting order must leave the active-order index.
                // A partially filled resting order keeps its FIFO position.
                if resting.is_filled() {
                    self.order_locations.remove(&resting.id);
                } else {
                    self.orders.insert(resting.id, resting);
                    level.order_ids.push_front(resting_id);
                    break;
                }
            }

            should_remove_level = level.order_ids.is_empty() || level.total_quantity == 0;
        }

        if should_remove_level {
            self.bids.remove(&price);
        }
    }
}
