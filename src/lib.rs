//! Core library for the limit order book.
//!
//! Keep the matching engine separate from `main.rs` so we can test it directly
//! and later reuse it from an API, CLI, benchmark, or market-data simulator.

pub mod domain;
pub mod engine;
pub mod error;
pub mod metrics;
pub mod replay;
pub mod simulator;

pub use domain::{BookEvent, BookSnapshot, Order, OrderId, Price, Quantity, Side, Trade};
pub use engine::{EventMode, OrderBook, OrderBookConfig};
pub use error::OrderBookError;
pub use metrics::{BookMetrics, TradeMetrics, calculate_book_metrics, calculate_trade_metrics};
pub use replay::{load_events_from_file, replay_events, save_events_to_file};
