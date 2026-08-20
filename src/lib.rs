//! A deterministic, single-writer limit order book.
//!
//! `domain` and `matching` are the pure matching core: synchronous, with no
//! knowledge of transport, concurrency, or configuration. `service` is the
//! sole owner of the Disruptor-backed, concurrency-safe engine handles that
//! `http` (and any future transport, such as an offline backtest runner) is
//! built on. `config` is the only module that reads the process environment.
//! `app` wires all of the above together.

pub mod app;
pub mod config;
pub mod domain;
pub mod http;
pub mod matching;
pub mod service;
