//! HTTP transport for the matching service.
//!
//! Handlers see only [`crate::service::ExchangeClient`] and the DTOs defined
//! in [`dto`]. They never import `crate::matching::{Engine, Exchange}` — the
//! disruptor-backed engine handles stay behind the `service` boundary.

mod dto;
mod error;
mod handlers;

pub use handlers::router;
