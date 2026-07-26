use serde::Serialize;
use thiserror::Error;

use crate::domain::{OrderError, OrderId, OrderStatus};

use super::{BookError, ExecutionReport};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RejectionReport {
    pub order_id: OrderId,
    pub status: OrderStatus,
    pub reason: RejectReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RejectReason {
    InvalidOrder(OrderError),
    DuplicateOrder(OrderId),
    BookFull,
    QuantityOverflow,
    ConstraintViolation(&'static str),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SubmitOutcome {
    Accepted(ExecutionReport),
    Rejected(RejectionReport),
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EngineFault {
    #[error("engine sequence exhausted")]
    SequenceExhausted,
    #[error("order book invariant violated: {0}")]
    Invariant(&'static str),
    #[error("order not found: {0}")]
    OrderNotFound(OrderId),
    #[error("book rejected an order after admission: {0}")]
    PostAdmission(#[source] BookError),
}

impl RejectionReport {
    pub(super) fn from_admission_error(
        order_id: OrderId,
        error: &BookError,
    ) -> Result<Self, EngineFault> {
        let reason = match error {
            BookError::InvalidOrder(error) => RejectReason::InvalidOrder(*error),
            BookError::DuplicateOrder(order_id) => RejectReason::DuplicateOrder(*order_id),
            BookError::Full => RejectReason::BookFull,
            BookError::QuantityOverflow => RejectReason::QuantityOverflow,
            BookError::InvalidConfiguration(message) => RejectReason::ConstraintViolation(message),
            BookError::Invariant(message) => return Err(EngineFault::Invariant(message)),
            BookError::SequenceExhausted => return Err(EngineFault::SequenceExhausted),
            BookError::OrderNotFound(order_id) => {
                return Err(EngineFault::OrderNotFound(*order_id));
            }
        };

        Ok(Self {
            order_id,
            status: OrderStatus::Rejected,
            reason,
        })
    }
}

impl EngineFault {
    pub(super) fn from_post_admission(error: BookError) -> Self {
        match error {
            BookError::Invariant(message) => Self::Invariant(message),
            BookError::SequenceExhausted => Self::SequenceExhausted,
            BookError::OrderNotFound(order_id) => Self::OrderNotFound(order_id),
            error => Self::PostAdmission(error),
        }
    }

    pub(super) fn from_cancel_error(error: BookError) -> Self {
        match error {
            BookError::OrderNotFound(order_id) => Self::OrderNotFound(order_id),
            BookError::Invariant(message) => Self::Invariant(message),
            error => Self::PostAdmission(error),
        }
    }
}
