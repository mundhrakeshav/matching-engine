use std::net::SocketAddr;

use ob::{api, domain::Symbol, matching::Exchange};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let address: SocketAddr = "0.0.0.0:8080".parse()?;
    let listener = TcpListener::bind(address).await?;
    println!("matching engine listening on {address}");
    let mut exchange = Exchange::new(100_000);
    exchange.prepare_symbols([Symbol::parse("BTC-USD").expect("valid configured symbol")]);
    let client = api::spawn_exchange(exchange, 1_024)?;
    axum::serve(listener, api::router(client))
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}
