use std::net::SocketAddr;

use polymarket_backend::{config::AppConfig, router::build_router};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = AppConfig::from_env()?;

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(config.rust_log.clone()))
        .init();

    let addr = SocketAddr::new(config.host.parse()?, config.port);
    let app = build_router(config);
    let listener = tokio::net::TcpListener::bind(addr).await?;

    tracing::info!(%addr, "starting polymarket backend");
    axum::serve(listener, app).await?;

    Ok(())
}
