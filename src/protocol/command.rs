use uuid::Uuid;
use crate::{OrderId, Price, Quantity, Side};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WireCommand {
    // Only Add carries a side; Cancel and Modify find the existing order.
    Add {
        instrument_id: Uuid,
        order_id: OrderId,
        side: Side,
        price: Price,
        quantity: Quantity,
    },
    Cancel {
        instrument_id: Uuid,
        order_id: OrderId,
    },
    Modify {
        instrument_id: Uuid,
        order_id: OrderId,
        price: Price,
        quantity: Quantity,
    },
}