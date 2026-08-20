//! Composition root.
//!
//! Loads configuration, wires the exchange, the concurrency-safe `service`
//! layer, and the `http` router together, then runs the server until
//! shutdown. This is the only module that constructs a
//! [`crate::matching::Exchange`] from configuration.

use std::net::SocketAddr;

use anyhow::Context;

use tokio::net::TcpListener;

use crate::{config::Config, http, matching::Exchange, service};

/// Runs the matching engine service to completion.
///
/// # Errors
///
/// Returns an error when configuration is invalid, the ring capacity is
/// rejected, or the HTTP listener cannot bind or serve.
pub async fn run() -> anyhow::Result<()> {
    let config = Config::load().context("failed to load configuration")?;

    let mut exchange = Exchange::new(config.book_capacity);
    exchange.prepare_symbols(config.symbols.clone());
    let client = service::spawn_exchange(exchange, config.ring_capacity)
        .context("failed to spawn exchange")?;

    let address = SocketAddr::new(config.host, config.port);
    let listener = TcpListener::bind(address)
        .await
        .with_context(|| format!("failed to bind {address}"))?;
    println!(
        "{} listening on {address} (log_level={})",
        config.service_name, config.log_level
    );

    axum::serve(listener, http::router(client))
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("matching engine server exited with an error")?;
    Ok(())
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}
