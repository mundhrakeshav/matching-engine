//! A deterministic, single-writer limit order book.
//!
//! The matching core is synchronous. Concurrency belongs at the service edge,
//! where a mutex serializes command application and keeps book invariants intact.

pub mod api;
pub mod config;
pub mod domain;
pub mod matching;
