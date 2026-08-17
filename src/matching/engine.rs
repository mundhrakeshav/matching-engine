use crate::domain::{Order, OrderId, Sequence};

use super::{Book, BookError, BookSnapshot, CancelReport, ExecutionReport, TopOfBook};

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
    ///
    /// # Errors
    ///
    /// Returns [`BookError::OrderNotFound`] when the order is not active.
    pub fn cancel(&mut self, order_id: OrderId) -> Result<CancelReport, BookError> {
        self.book.cancel(order_id)
    }

    /// Returns the best bid and best ask without exposing mutable book state.
    #[must_use]
    pub fn top_of_book(&self) -> TopOfBook {
        self.book.top_of_book()
    }

    /// Returns up to `levels` best price levels on each side.
    #[must_use]
    pub fn depth(&self, levels: usize) -> BookSnapshot {
        self.book.depth(levels)
    }

    /// Returns all active price levels, best first on each side.
    #[must_use]
    pub fn snapshot(&self) -> BookSnapshot {
        self.book.snapshot()
    }
}
