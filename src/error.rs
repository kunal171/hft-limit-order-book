use serde::{Deserialize, Serialize};
use std::fmt;

/// Errors returned by the order book API.
///
/// The matching logic currently returns `Ok(trades)` for valid orders. These
/// variants are here so the next phase can add input validation without changing
/// the public return type again.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderBookError {
    ZeroQuantity,
    DuplicateOrderId,
    UnknownOrderId,
}

impl fmt::Display for OrderBookError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroQuantity => write!(f, "order quantity must be greater than zero"),
            Self::DuplicateOrderId => write!(f, "order id already exists"),
            Self::UnknownOrderId => write!(f, "order id does not exist"),
        }
    }
}

impl std::error::Error for OrderBookError {}
