use std::net::SocketAddr;

use ob::{api, matching::Engine};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let address: SocketAddr = "0.0.0.0:8080".parse()?;
    let listener = TcpListener::bind(address).await?;
    println!("matching engine listening on {address}");
    axum::serve(listener, api::router(Engine::new(100_000)))
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}
