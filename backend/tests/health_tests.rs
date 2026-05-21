use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use polymarket_backend::{
    config::AppConfig,
    router::{build_router, build_router_with_storage},
    storage::{
        NewNotificationDelivery, NewRawMarketEvent, NewRuntimeEvent, NewSignal, NewTick,
        RawMarketEventRecord, ReplayTick, RuntimeEventRecord, SignalRecord, StorageCommand,
        StorageError, StorageRepository, StorageWriter, StorageWriterRuntime, TickRecord,
    },
};
use serde_json::{json, Value};
use std::{sync::Arc, time::Duration};
use time::OffsetDateTime;
use tower::ServiceExt;
use uuid::Uuid;

#[tokio::test]
async fn healthz_returns_structured_service_status() {
    let config = AppConfig::from_env_map([
        ("POLY_ENV".to_string(), "local".to_string()),
        ("APP_VERSION".to_string(), "test-version".to_string()),
        ("SUPPORTED_MARKETS".to_string(), "btc5m,eth15m".to_string()),
    ])
    .expect("test config should be valid");
    let app = build_router(config);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/healthz")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("health request should be handled");

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body should be readable");
    let json: Value = serde_json::from_slice(&body).expect("response should be json");

    assert_eq!(
        json,
        json!({
            "status": "ok",
            "service": "polymarket-backend",
            "version": "test-version",
            "build": {
                "version": "test-version"
            },
            "environment": "local",
            "database_configured": false,
            "supported_markets": ["btc5m", "eth15m"],
            "storage_writer": null,
            "runtime": {
                "environment": "local",
                "database_configured": false,
                "supported_markets": ["btc5m", "eth15m"]
            }
        })
    );
}

#[tokio::test]
async fn healthz_exposes_storage_writer_metrics_when_configured() {
    let config = AppConfig::from_env_map([
        ("POLY_ENV".to_string(), "local".to_string()),
        ("APP_VERSION".to_string(), "test-version".to_string()),
        (
            "DATABASE_URL".to_string(),
            "postgres://configured".to_string(),
        ),
    ])
    .expect("test config should be valid");
    let (writer, join) =
        StorageWriter::spawn(Arc::new(HealthRepository), 4, Duration::from_millis(5));
    writer
        .try_enqueue(StorageCommand::RuntimeEvent(NewRuntimeEvent {
            component: "health-test".to_string(),
            severity: "info".to_string(),
            event_type: "probe".to_string(),
            message: "probe".to_string(),
            details: json!({}),
        }))
        .expect("enqueue should succeed");

    tokio::time::sleep(Duration::from_millis(20)).await;
    let runtime = StorageWriterRuntime::new(writer.clone(), join);
    let app = build_router_with_storage(config, Some(runtime));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/healthz")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("health request should be handled");

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body should be readable");
    let json: Value = serde_json::from_slice(&body).expect("response should be json");

    assert_eq!(json["database_configured"], true);
    assert_eq!(json["storage_writer"]["queued_capacity"], 4);
    assert_eq!(json["storage_writer"]["task_status"], "running");
    assert_eq!(json["storage_writer"]["accepted"], 1);
    assert_eq!(json["storage_writer"]["written"], 1);

    drop(writer);
}

struct HealthRepository;

#[async_trait::async_trait]
impl StorageRepository for HealthRepository {
    async fn insert_raw_market_event(
        &self,
        _event: &NewRawMarketEvent,
    ) -> Result<RawMarketEventRecord, StorageError> {
        unreachable!("health test does not insert raw events")
    }

    async fn insert_tick(&self, _tick: &NewTick) -> Result<TickRecord, StorageError> {
        unreachable!("health test does not insert ticks")
    }

    async fn insert_signal(&self, _signal: &NewSignal) -> Result<SignalRecord, StorageError> {
        unreachable!("health test does not insert signals")
    }

    async fn insert_notification_delivery(
        &self,
        _delivery: &NewNotificationDelivery,
    ) -> Result<Uuid, StorageError> {
        unreachable!("health test does not insert notifications")
    }

    async fn insert_runtime_event(
        &self,
        event: &NewRuntimeEvent,
    ) -> Result<RuntimeEventRecord, StorageError> {
        Ok(RuntimeEventRecord {
            id: Uuid::new_v4(),
            component: event.component.clone(),
            severity: event.severity.clone(),
            event_type: event.event_type.clone(),
            message: event.message.clone(),
            details: event.details.clone(),
            created_at: OffsetDateTime::UNIX_EPOCH,
        })
    }

    async fn replay_ticks_for_window(
        &self,
        _market_key: &str,
        _window_start: OffsetDateTime,
    ) -> Result<Vec<ReplayTick>, StorageError> {
        unreachable!("health test does not replay ticks")
    }
}
