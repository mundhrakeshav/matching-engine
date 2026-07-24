use serde::Serialize;

use super::{OrderId, OrderSide, Price, Quantity, Sequence, UserId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Trade {
    pub sequence: Sequence,
    pub maker_order_id: OrderId,
    pub taker_order_id: OrderId,
    pub maker_id: UserId,
    pub taker_id: UserId,
    pub taker_side: OrderSide,
    pub quantity: Quantity,
    pub price: Price,
}
