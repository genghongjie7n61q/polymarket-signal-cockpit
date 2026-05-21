use std::{net::SocketAddr, sync::Arc, time::Duration};

use polymarket_backend::{
    config::AppConfig,
    realtime::RealtimeRuntime,
    router::build_router_with_runtime,
    storage::{
        connect_pool, run_migrations, PgPoolOptionsConfig, PostgresStorage, StorageWriter,
        StorageWriterRuntime,
    },
};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = AppConfig::from_env()?;

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(config.rust_log.clone()))
        .init();

    let addr = SocketAddr::new(config.host.parse()?, config.port);
    let storage_writer = if let Some(database_url) = config.database_url.as_deref() {
        let pool = connect_pool(database_url, PgPoolOptionsConfig::default()).await?;
        run_migrations(&pool).await?;
        let storage = Arc::new(PostgresStorage::new(pool));
        let (writer, join) = StorageWriter::spawn(
            storage,
            config.storage_writer_queue_capacity,
            Duration::from_millis(config.storage_writer_flush_interval_ms),
        );
        Some(StorageWriterRuntime::new(writer, join))
    } else {
        None
    };
    let realtime = Some(RealtimeRuntime::spawn_default());
    let app = build_router_with_runtime(config, storage_writer, realtime);
    let listener = tokio::net::TcpListener::bind(addr).await?;

    tracing::info!(%addr, "starting polymarket backend");
    axum::serve(listener, app).await?;

    Ok(())
}
