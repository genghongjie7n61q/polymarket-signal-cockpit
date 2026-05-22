use std::sync::Arc;

use axum::{routing::get, Router};

use crate::{
    api::api_router,
    config::AppConfig,
    health::healthz,
    realtime::RealtimeRuntime,
    storage::{StorageRepository, StorageWriterRuntime},
};

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub storage_writer: Option<StorageWriterRuntime>,
    pub realtime: Option<RealtimeRuntime>,
    pub storage: Option<Arc<dyn StorageRepository>>,
}

pub fn build_router(config: AppConfig) -> Router {
    build_router_with_storage(config, None)
}

pub fn build_router_with_storage(
    config: AppConfig,
    storage_writer: Option<StorageWriterRuntime>,
) -> Router {
    build_router_with_runtime(config, storage_writer, None)
}

pub fn build_router_with_runtime(
    config: AppConfig,
    storage_writer: Option<StorageWriterRuntime>,
    realtime: Option<RealtimeRuntime>,
) -> Router {
    build_router_with_runtime_and_storage(config, storage_writer, realtime, None)
}

pub fn build_router_with_runtime_and_storage(
    config: AppConfig,
    storage_writer: Option<StorageWriterRuntime>,
    realtime: Option<RealtimeRuntime>,
    storage: Option<Arc<dyn StorageRepository>>,
) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .nest("/api", api_router())
        .with_state(AppState {
            config: Arc::new(config),
            storage_writer,
            realtime,
            storage,
        })
}
