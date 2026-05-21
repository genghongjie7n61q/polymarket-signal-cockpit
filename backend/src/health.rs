use axum::{extract::State, Json};
use serde::Serialize;

use crate::{config::AppConfig, router::AppState, storage::StorageWriterSnapshot};

const SERVICE_NAME: &str = "polymarket-backend";

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    status: &'static str,
    service: &'static str,
    version: String,
    build: BuildInfo,
    environment: &'static str,
    database_configured: bool,
    supported_markets: Vec<String>,
    storage_writer: Option<StorageWriterSnapshot>,
    runtime: RuntimeInfo,
}

#[derive(Debug, Serialize)]
struct BuildInfo {
    version: String,
}

#[derive(Debug, Serialize)]
struct RuntimeInfo {
    environment: &'static str,
    database_configured: bool,
    supported_markets: Vec<String>,
}

pub async fn healthz(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse::from_state(&state))
}

impl HealthResponse {
    fn from_state(state: &AppState) -> Self {
        let config = &state.config;
        let version = config.app_version.clone();
        let environment = config.environment.as_str();
        let database_configured = config.database_configured();
        let supported_markets = config.supported_markets.clone();
        let storage_writer = state.storage_writer.as_ref().map(|writer| writer.snapshot());

        Self {
            status: "ok",
            service: SERVICE_NAME,
            version: version.clone(),
            build: BuildInfo { version },
            environment,
            database_configured,
            supported_markets: supported_markets.clone(),
            storage_writer,
            runtime: RuntimeInfo {
                environment,
                database_configured,
                supported_markets,
            },
        }
    }
}
