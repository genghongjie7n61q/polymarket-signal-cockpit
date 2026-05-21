use std::{collections::BTreeMap, net::SocketAddr, sync::Arc, time::Duration};

use polymarket_backend::{
    config::AppConfig,
    realtime::{
        run_coinbase_ws_collector_until, CoinbaseCollectorConfig, MarketKey, RealtimeRuntime,
        DEFAULT_REALTIME_QUEUE_CAPACITY,
    },
    router::build_router_with_runtime,
    storage::{
        connect_pool, run_migrations, PgPoolOptionsConfig, PostgresStorage, StorageWriter,
        StorageWriterRuntime,
    },
};
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = AppConfig::from_env()?;

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(config.rust_log.clone()))
        .init();

    let addr = SocketAddr::new(config.host.parse()?, config.port);
    let (storage_writer, market_ids) = if let Some(database_url) = config.database_url.as_deref() {
        let pool = connect_pool(database_url, PgPoolOptionsConfig::default()).await?;
        run_migrations(&pool).await?;
        let storage = PostgresStorage::new(pool);
        let market_ids = storage.load_realtime_market_ids().await?;
        let storage = Arc::new(storage);
        let (writer, join) = StorageWriter::spawn(
            storage,
            config.storage_writer_queue_capacity,
            Duration::from_millis(config.storage_writer_flush_interval_ms),
        );
        (Some(StorageWriterRuntime::new(writer, join)), market_ids)
    } else {
        (None, BTreeMap::<MarketKey, Uuid>::new())
    };
    let realtime = Some(match storage_writer.as_ref() {
        Some(writer) => RealtimeRuntime::spawn_with_storage(
            DEFAULT_REALTIME_QUEUE_CAPACITY,
            writer.handle().clone(),
            market_ids,
        ),
        None => RealtimeRuntime::spawn_default(),
    });
    if let Some(runtime) = realtime.clone() {
        tokio::spawn(async move {
            loop {
                tracing::info!("starting coinbase websocket collector");
                let mut coinbase_config = CoinbaseCollectorConfig::default();
                coinbase_config.proxy = std::env::var("COINBASE_WS_PROXY")
                    .ok()
                    .filter(|value| !value.trim().is_empty());
                let result =
                    run_coinbase_ws_collector_until(coinbase_config, runtime.clone(), usize::MAX)
                        .await;
                match result {
                    Ok(published) => {
                        tracing::warn!(published, "coinbase websocket collector stopped");
                    }
                    Err(error) => {
                        tracing::warn!(%error, "coinbase websocket collector failed");
                    }
                }
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        });
    }
    let app = build_router_with_runtime(config, storage_writer, realtime);
    let listener = tokio::net::TcpListener::bind(addr).await?;

    tracing::info!(%addr, "starting polymarket backend");
    axum::serve(listener, app).await?;

    Ok(())
}
