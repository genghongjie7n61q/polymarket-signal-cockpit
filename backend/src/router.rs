use std::sync::Arc;

use axum::{routing::get, Router};

use crate::{config::AppConfig, health::healthz};

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
}

pub fn build_router(config: AppConfig) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .with_state(AppState {
            config: Arc::new(config),
        })
}
