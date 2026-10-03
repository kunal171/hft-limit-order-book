use crate::domain::{Order, Price, Side, Trade};

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

            self.match_at_level(Side::Sell, best_ask_price, &mut incoming, &mut trades);
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

            self.match_at_level(Side::Buy, best_bid_price, &mut incoming, &mut trades);
        }

        // Any unfilled quantity becomes a resting ask.
        if !incoming.is_filled() {
            // Only resting orders belong in the active-order index.
            self.rest_order(incoming);
        }

        trades
    }

    /// Match the incoming order against the queue at one price.
    ///
    /// `maker_side` is the side of the resting orders being consumed.
    fn match_at_level(
        &mut self,
        maker_side: Side,
        price: Price,
        incoming: &mut Order,
        trades: &mut Vec<Trade>,
    ) {
        let levels = match maker_side {
            Side::Buy => &mut self.bids,
            Side::Sell => &mut self.asks,
        };

        let Some(level) = levels.get_mut(&price) else {
            return;
        };

        while !incoming.is_filled() {
            let Some(slot) = level.head else {
                break;
            };

            let resting = &mut self.arena.get_mut(slot).order;
            let traded_qty = incoming.remaining_qty.min(resting.remaining_qty);
            incoming.remaining_qty -= traded_qty;

            trades.push(Trade::new(resting.id, incoming.id, price, traded_qty));

            if traded_qty == resting.remaining_qty {
                // Fully filled: `unlink` subtracts its quantity from the level.
                let resting_id = resting.id;
                level.unlink(&mut self.arena, slot);
                self.arena.remove(slot);
                self.order_slots.remove(&resting_id);
            } else {
                // Partially filled: it stays at the head and keeps its priority.
                resting.remaining_qty -= traded_qty;
                level.total_quantity -= traded_qty;
            }
        }

        if level.is_empty() {
            levels.remove(&price);
        }
    }
}
