use crate::domain::{Order, OrderId, Sequence};

use super::{Book, BookError, EngineFault, RejectionReport, SubmitOutcome};

/// The sole command boundary. Every accepted order receives one monotonic sequence.
#[derive(Debug)]
pub struct Engine {
    book: Book,
    sequence: Sequence,
}

impl Engine {
    pub fn new(book_capacity: usize) -> Self {
        Self {
            book: Book::new(book_capacity),
            sequence: Sequence::from(0),
        }
    }

    /// Applies one new order at the next deterministic sequence.
    ///
    /// # Errors
    ///
    /// Expected admission failures are returned as
    /// [`SubmitOutcome::Rejected`]. `Err` is reserved for engine faults.
    pub fn submit(&mut self, order: Order) -> Result<SubmitOutcome, EngineFault> {
        if let Err(error) = self.validate_submission(&order) {
            return Ok(SubmitOutcome::Rejected(
                RejectionReport::from_admission_error(order.resting.id, &error)?,
            ));
        }

        let next_sequence = self
            .sequence
            .checked_add(1)
            .ok_or(EngineFault::SequenceExhausted)?;

        match self.book.submit(order, next_sequence) {
            Ok(report) => {
                self.sequence = next_sequence;
                Ok(SubmitOutcome::Accepted(report))
            }
            Err(error) => Err(EngineFault::from_post_admission(error)),
        }
    }

    /// Cancels an active resting order through the engine command boundary.
    ///
    /// A successful cancellation consumes the next engine sequence. Missing
    /// or already completed orders are rejected without advancing it.
    pub fn cancel(&mut self, order_id: OrderId) -> Result<Order, EngineFault> {
        let next_sequence = self
            .sequence
            .checked_add(1)
            .ok_or(EngineFault::SequenceExhausted)?;
        let cancelled = self
            .book
            .cancel(order_id)
            .map_err(EngineFault::from_cancel_error)?;

        self.sequence = next_sequence;

        Ok(cancelled)
    }

    fn validate_submission(&self, order: &Order) -> Result<(), BookError> {
        order.validate()?;
        self.book.validate_order_constraints(order)?;
        self.book.validate_order_id_available(order.resting.id)?;
        self.book.validate_resting_capacity(order)
    }
}
