use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderStatus {
    New,
    Accepted,
    Rejected,
    PartiallyFilled,
    Filled,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("invalid order status transition: {from:?} -> {to:?}")]
pub struct StatusTransitionError {
    pub from: OrderStatus,
    pub to: OrderStatus,
}

impl OrderStatus {
    /// Advances this status when the requested lifecycle transition is valid.
    ///
    /// # Errors
    ///
    /// Returns [`StatusTransitionError`] when the requested transition is not
    /// permitted by the order lifecycle.
    pub fn transition_to(&mut self, next: Self) -> Result<(), StatusTransitionError> {
        let valid = matches!(
            (*self, next),
            (Self::New, Self::Accepted | Self::Rejected)
                | (
                    Self::Accepted | Self::PartiallyFilled,
                    Self::PartiallyFilled | Self::Filled | Self::Cancelled
                )
        );

        if !valid {
            return Err(StatusTransitionError {
                from: *self,
                to: next,
            });
        }

        *self = next;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{OrderStatus, StatusTransitionError};

    #[test]
    fn valid_lifecycle_transitions_are_allowed() {
        let valid = [
            (OrderStatus::New, OrderStatus::Accepted),
            (OrderStatus::New, OrderStatus::Rejected),
            (OrderStatus::Accepted, OrderStatus::PartiallyFilled),
            (OrderStatus::Accepted, OrderStatus::Filled),
            (OrderStatus::Accepted, OrderStatus::Cancelled),
            (OrderStatus::PartiallyFilled, OrderStatus::PartiallyFilled),
            (OrderStatus::PartiallyFilled, OrderStatus::Filled),
            (OrderStatus::PartiallyFilled, OrderStatus::Cancelled),
        ];

        for (from, to) in valid {
            let mut status = from;
            assert_eq!(status.transition_to(to), Ok(()));
            assert_eq!(status, to);
        }
    }

    #[test]
    fn terminal_and_invalid_transitions_are_rejected() {
        let invalid = [
            (OrderStatus::New, OrderStatus::Filled),
            (OrderStatus::Accepted, OrderStatus::Rejected),
            (OrderStatus::PartiallyFilled, OrderStatus::Accepted),
            (OrderStatus::Filled, OrderStatus::Cancelled),
            (OrderStatus::Cancelled, OrderStatus::Accepted),
            (OrderStatus::Rejected, OrderStatus::Accepted),
        ];

        for (from, to) in invalid {
            let mut status = from;
            assert_eq!(
                status.transition_to(to),
                Err(StatusTransitionError { from, to })
            );
            assert_eq!(status, from);
        }
    }
}
