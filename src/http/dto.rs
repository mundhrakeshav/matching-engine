//! Wire types for the matching HTTP API.
//!
//! Handlers translate between these shapes and [`crate::service`] calls.
//! Nothing here imports [`crate::matching::Engine`] or
//! [`crate::matching::Exchange`] — only read-only view types
//! ([`PriceLevelView`]) that `crate::matching` already exposes as part of its
//! public, transport-agnostic API.

use serde::{Deserialize, Serialize};

use crate::{domain::Order, matching::PriceLevelView, service::BookView};

#[derive(Debug, Deserialize)]
pub struct BookQuery {
    pub symbol: String,
    pub depth: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SymbolQuery {
    pub symbol: String,
}

#[derive(Debug, Deserialize)]
pub struct SubmitRequest {
    pub symbol: String,
    #[serde(flatten)]
    pub order: Order,
}

#[derive(Debug, Serialize)]
pub struct BookResponse {
    pub symbol: String,
    pub depth: String,
    pub bids: Vec<PriceLevelView>,
    pub asks: Vec<PriceLevelView>,
}

impl BookResponse {
    pub(super) fn from_view(symbol: String, depth: String, view: BookView) -> Self {
        let (bids, asks) = match view {
            BookView::Top(top) => (top.bid.into_iter().collect(), top.ask.into_iter().collect()),
            BookView::Snapshot(snapshot) => (snapshot.bids, snapshot.asks),
        };
        Self {
            symbol,
            depth,
            bids,
            asks,
        }
    }
}
