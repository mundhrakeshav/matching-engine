use std::collections::HashMap;

use thiserror::Error;

use crate::domain::{Order, OrderId, Symbol};

use super::{BookError, BookSnapshot, CancelReport, Engine, ExecutionReport, TopOfBook};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ExchangeError {
    #[error("unknown symbol: {0}")]
    UnknownSymbol(Symbol),
    #[error(transparent)]
    Book(#[from] BookError),
}

/// Routes commands to one independent matching engine per instrument.
#[derive(Debug)]
pub struct Exchange {
    books: HashMap<Symbol, Engine>,
    book_capacity: usize,
}

impl Exchange {
    pub fn new(book_capacity: usize) -> Self {
        Self {
            books: HashMap::new(),
            book_capacity,
        }
    }

    pub fn prepare_symbols<I>(&mut self, symbols: I)
    where
        I: IntoIterator<Item = Symbol>,
    {
        for symbol in symbols {
            self.books
                .entry(symbol)
                .or_insert_with(|| Engine::new(self.book_capacity));
        }
    }

    pub(crate) fn into_books(self) -> HashMap<Symbol, Engine> {
        self.books
    }

    /// # Errors
    ///
    /// Returns [`ExchangeError::UnknownSymbol`] when the instrument is not configured.
    pub fn submit(
        &mut self,
        symbol: Symbol,
        order: Order,
    ) -> Result<ExecutionReport, ExchangeError> {
        self.books
            .get_mut(&symbol)
            .ok_or(ExchangeError::UnknownSymbol(symbol))?
            .submit(order)
            .map_err(ExchangeError::from)
    }

    /// # Errors
    ///
    /// Returns [`ExchangeError::UnknownSymbol`] when the instrument is not configured.
    pub fn cancel(
        &mut self,
        symbol: Symbol,
        order_id: OrderId,
    ) -> Result<CancelReport, ExchangeError> {
        self.books
            .get_mut(&symbol)
            .ok_or(ExchangeError::UnknownSymbol(symbol))?
            .cancel(order_id)
            .map_err(ExchangeError::from)
    }

    /// # Errors
    ///
    /// Returns [`ExchangeError::UnknownSymbol`] when the instrument is not configured.
    pub fn top_of_book(&self, symbol: Symbol) -> Result<TopOfBook, ExchangeError> {
        Ok(self
            .books
            .get(&symbol)
            .ok_or(ExchangeError::UnknownSymbol(symbol))?
            .top_of_book())
    }

    /// # Errors
    ///
    /// Returns [`ExchangeError::UnknownSymbol`] when the instrument is not configured.
    pub fn depth(&self, symbol: Symbol, levels: usize) -> Result<BookSnapshot, ExchangeError> {
        Ok(self
            .books
            .get(&symbol)
            .ok_or(ExchangeError::UnknownSymbol(symbol))?
            .depth(levels))
    }

    /// # Errors
    ///
    /// Returns [`ExchangeError::UnknownSymbol`] when the instrument is not configured.
    pub fn snapshot(&self, symbol: Symbol) -> Result<BookSnapshot, ExchangeError> {
        Ok(self
            .books
            .get(&symbol)
            .ok_or(ExchangeError::UnknownSymbol(symbol))?
            .snapshot())
    }
}
