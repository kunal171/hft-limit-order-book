//! Matching engine modules.
//!
//! `order_book` exposes the public engine API, while `matching` keeps the
//! private price-time priority matching helpers out of the main API file.

mod config;
mod matching;
mod order_book;

pub use config::{EventMode, OrderBookConfig};
pub use order_book::OrderBook;
