use thiserror::Error;

use crate::{domain::Symbol, matching::BookError};

/// Failure returned by [`super::ExchangeClient`] operations.
#[derive(Debug, Error)]
pub enum ServiceError {
    #[error("unknown symbol: {0}")]
    UnknownSymbol(Symbol),
    #[error("matching engine ring is full")]
    Overloaded,
    #[error("matching engine is unavailable")]
    Unavailable,
    #[error(transparent)]
    Book(#[from] BookError),
}
