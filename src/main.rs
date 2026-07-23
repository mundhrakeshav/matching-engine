use std::net::SocketAddr;

use ob::{
    api::{AppState, router},
    config::Config,
};
use tokio::net::TcpListener;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::load()?;
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(EnvFilter::try_new(&config.log_filter)?)
        .init();

    let address = SocketAddr::new(config.host, config.port);
    let listener = TcpListener::bind(address).await?;
    info!(%address, service = %config.service_name, "starting application");
    axum::serve(
        listener,
        router(AppState::new(config.service_name, config.book_capacity)),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;
    info!("application stopped");
    Ok(())
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install Ctrl+C handler");
}
