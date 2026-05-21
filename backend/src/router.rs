use std::sync::Arc;

use axum::{routing::get, Router};

use crate::{config::AppConfig, health::healthz, storage::StorageWriterHandle};

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub storage_writer: Option<StorageWriterHandle>,
}

pub fn build_router(config: AppConfig) -> Router {
    build_router_with_storage(config, None)
}

pub fn build_router_with_storage(
    config: AppConfig,
    storage_writer: Option<StorageWriterHandle>,
) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .with_state(AppState {
            config: Arc::new(config),
            storage_writer,
        })
}
