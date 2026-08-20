//! Owns concurrency-safe access to the matching core.
//!
//! [`ExchangeClient`] is the *only* type outside `crate::matching` permitted
//! to hold a [`crate::matching::Engine`] or [`crate::matching::Exchange`].
//! Transport layers — today `crate::http`, eventually an offline backtest
//! runner — call the methods below and never see the engine directly.

mod client;
mod error;

pub use client::{BookDepth, BookView, ExchangeClient, SpawnError, spawn_exchange};
pub use error::ServiceError;
