//! A deterministic, single-writer limit order book.
//!
//! The matching core applies commands synchronously on one worker per engine.
//! Concurrent callers enter through each engine's bounded command queue.

pub mod domain;
pub mod matching;
