use crate::domain::{Order, Sequence};

use super::{Book, BookError, ExecutionReport, OrderBookSnapshot};

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
            sequence: 0,
        }
    }

    /// Applies one new order at the next deterministic sequence.
    ///
    /// # Errors
    ///
    /// Returns an error when the sequence space is exhausted or the book rejects
    /// the order.
    pub fn submit(&mut self, order: Order) -> Result<ExecutionReport, BookError> {
        self.sequence = self
            .sequence
            .checked_add(1)
            .ok_or(BookError::SequenceExhausted)?;
        order.validate()?;
        self.book.submit(order, self.sequence)
    }

    /// Applies one cancellation at the next deterministic sequence.
    ///
    /// # Errors
    ///
    /// Returns an error when the sequence space is exhausted or the order is not
    /// currently resting.
    pub fn cancel(&mut self, order_id: u64) -> Result<(), BookError> {
        self.sequence = self
            .sequence
            .checked_add(1)
            .ok_or(BookError::SequenceExhausted)?;
        self.book.cancel(order_id)
    }

    pub fn snapshot(&self) -> OrderBookSnapshot {
        self.book.snapshot(self.sequence)
    }
}
