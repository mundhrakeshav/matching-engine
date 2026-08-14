use crate::domain::{Order, Sequence};

use super::{Book, BookError, CancelReport, ExecutionReport};

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
    /// Returns an error when the sequence space is exhausted or the book rejects
    /// the order.
    pub fn submit(&mut self, order: Order) -> Result<ExecutionReport, BookError> {
        let next_sequence = self
            .sequence
            .checked_add(1)
            .ok_or(BookError::SequenceExhausted)?;
        let report = self.book.submit(order, next_sequence)?;
        self.sequence = next_sequence;
        Ok(report)
    }

    /// Cancels an active resting order.
    pub fn cancel(&mut self, order_id: crate::domain::OrderId) -> Result<CancelReport, BookError> {
        self.book.cancel(order_id)
    }
}
