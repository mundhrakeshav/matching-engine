use crate::domain::status::OrderStatus;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{OrderId, Price, Quantity, Sequence, UserId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderKind {
    Market,
    Limit,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestingOrder {
    pub id: OrderId,
    pub user_id: UserId,
    pub original_qty: Quantity,
    pub open_qty: Quantity,
    pub accepted_sequence: Sequence,
    pub status: OrderStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Order {
    #[serde(flatten)]
    pub resting: RestingOrder,
    pub limit_price: Option<Price>,
    pub kind: OrderKind,
    pub side: OrderSide,
}

#[derive(Debug, Clone, Copy, Error, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderError {
    #[error("quantity must be positive")]
    ZeroQuantity,
    #[error("open quantity must be positive and no greater than original quantity")]
    InvalidOpenQuantity,
    #[error("limit orders require a positive price")]
    InvalidLimitPrice,
    #[error("new orders must have new status")]
    InvalidInitialStatus,
}

impl Order {
    /// Validates the order's quantity and price constraints.
    ///
    /// # Errors
    ///
    /// Returns an error when the order cannot be accepted by the matching core.
    pub fn validate(&self) -> Result<(), OrderError> {
        if self.resting.status != OrderStatus::New {
            return Err(OrderError::InvalidInitialStatus);
        }
        if self.resting.original_qty == Quantity::from(0) {
            return Err(OrderError::ZeroQuantity);
        }
        if self.resting.open_qty == Quantity::from(0)
            || self.resting.open_qty > self.resting.original_qty
        {
            return Err(OrderError::InvalidOpenQuantity);
        }
        if self.kind == OrderKind::Limit
            && self.limit_price.is_none_or(|price| price <= Price::from(0))
        {
            return Err(OrderError::InvalidLimitPrice);
        }
        Ok(())
    }

    pub fn crosses(&self, maker_price: Price) -> bool {
        match self.side {
            OrderSide::Buy => self.limit_price.is_some_and(|price| price >= maker_price),
            OrderSide::Sell => self.limit_price.is_some_and(|price| price <= maker_price),
        }
    }
}
