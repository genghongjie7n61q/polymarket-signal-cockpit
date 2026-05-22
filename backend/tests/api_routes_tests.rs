use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use polymarket_backend::{
    config::AppConfig,
    realtime::{
        MarketKey, MarketTick, RealtimeBus, RealtimeEvent, RealtimeRuntime, RealtimeStateOwner,
    },
    router::{build_router_with_runtime, build_router_with_runtime_and_storage},
    storage::{
        CandleRecord, NewNotificationDelivery, NewRawMarketEvent, NewRuntimeEvent, NewSignal,
        NewTick, RawMarketEventRecord, ReplayTick, RuntimeEventRecord, SignalRecord,
        SignalWithMarketRecord, StorageError, StorageRepository, TickRecord,
    },
};
use serde_json::Value;
use std::{sync::Arc, time::Duration};
use time::OffsetDateTime;
use tower::ServiceExt;
use uuid::Uuid;

#[tokio::test]
async fn markets_api_returns_supported_markets_with_live_state() {
    let app = build_test_app_with_tick().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/markets")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("request should be handled");

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body should be readable");
    let json: Value = serde_json::from_slice(&body).expect("response should be json");

    assert_eq!(json["markets"].as_array().expect("markets array").len(), 2);
    assert_eq!(json["markets"][0]["market_key"], "btc5m");
    assert_eq!(json["markets"][0]["symbol"], "BTC-USD");
    assert_eq!(json["markets"][0]["interval_seconds"], 300);
    assert_eq!(json["markets"][0]["source_status"], "stale");
    assert_eq!(json["markets"][0]["latest_tick"]["price"], "101.25");
    assert_eq!(json["markets"][1]["market_key"], "eth15m");
}

#[tokio::test]
async fn market_state_api_returns_tick_window_snapshot_and_recent_candles() {
    let app = build_test_app_with_tick().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/markets/btc5m/state")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("request should be handled");

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body should be readable");
    let json: Value = serde_json::from_slice(&body).expect("response should be json");

    assert_eq!(json["market_key"], "btc5m");
    assert_eq!(json["latest_tick"]["symbol"], "BTC-USD");
    assert_eq!(json["current_window"]["event_slug"], "btc-updown-5m-0");
    assert_eq!(json["latest_snapshot"], Value::Null);
    assert_eq!(json["recent_candles"].as_array().expect("candles").len(), 1);
    assert_eq!(json["recent_candles"][0]["close"], "101.25");
}

#[tokio::test]
async fn market_state_api_rejects_unknown_market() {
    let app = build_test_app_with_tick().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/markets/sol5m/state")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("request should be handled");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn runtime_health_api_returns_runtime_snapshot() {
    let app = build_test_app_with_tick().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/runtime/health")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("request should be handled");

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body should be readable");
    let json: Value = serde_json::from_slice(&body).expect("response should be json");

    assert_eq!(json["realtime"]["bus"]["accepted"], 1);
    assert_eq!(json["realtime"]["state"]["metrics"]["processed"], 1);
}

#[tokio::test]
async fn candles_api_returns_recent_persisted_candles_for_market() {
    let app = build_test_app_with_repository(ApiRepository {
        candles: vec![
            CandleRecord {
                market_key: "btc5m".to_string(),
                start_ts: OffsetDateTime::UNIX_EPOCH + time::Duration::seconds(60),
                open: "100.0".parse().unwrap(),
                high: "103.0".parse().unwrap(),
                low: "99.0".parse().unwrap(),
                close: "102.5".parse().unwrap(),
                volume: "4.2".parse().unwrap(),
            },
            CandleRecord {
                market_key: "btc5m".to_string(),
                start_ts: OffsetDateTime::UNIX_EPOCH + time::Duration::seconds(120),
                open: "102.5".parse().unwrap(),
                high: "104.0".parse().unwrap(),
                low: "101.0".parse().unwrap(),
                close: "103.5".parse().unwrap(),
                volume: "3.8".parse().unwrap(),
            },
        ],
        signals: Vec::new(),
    })
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/markets/btc5m/candles?limit=2")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("request should be handled");

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body should be readable");
    let json: Value = serde_json::from_slice(&body).expect("response should be json");

    assert_eq!(json["market_key"], "btc5m");
    assert_eq!(json["candles"].as_array().expect("candles").len(), 2);
    assert_eq!(json["candles"][0]["close"], "102.5");
    assert_eq!(json["candles"][1]["volume"], "3.8");
}

#[tokio::test]
async fn signals_api_returns_latest_persisted_signals_for_market() {
    let app = build_test_app_with_repository(ApiRepository {
        candles: Vec::new(),
        signals: vec![SignalWithMarketRecord {
            id: Uuid::new_v4(),
            market_key: "btc5m".to_string(),
            market_window_id: Uuid::new_v4(),
            model_version_id: Uuid::new_v4(),
            signal_type: "actionable_alert".to_string(),
            side: Some("Up".to_string()),
            confidence: Some("0.82".parse().unwrap()),
            limit_price: Some("0.51".parse().unwrap()),
            suggested_size: Some("2.5".parse().unwrap()),
            ttl_ms: Some(15_000),
            reason: "positive edge".to_string(),
            features: serde_json::json!({"edge_bps": 6}),
            input_snapshot_hash: "snapshot-1".to_string(),
            created_at: OffsetDateTime::UNIX_EPOCH + time::Duration::seconds(90),
        }],
    })
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/signals?market_key=btc5m&limit=20")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("request should be handled");

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body should be readable");
    let json: Value = serde_json::from_slice(&body).expect("response should be json");

    assert_eq!(json["market_key"], "btc5m");
    assert_eq!(json["signals"].as_array().expect("signals").len(), 1);
    assert_eq!(json["signals"][0]["side"], "Up");
    assert_eq!(json["signals"][0]["confidence"], "0.82");
    assert_eq!(json["signals"][0]["limit_price"], "0.51");
    assert_eq!(json["signals"][0]["reason"], "positive edge");
}

async fn build_test_app_with_tick() -> axum::Router {
    let config = AppConfig::from_env_map([
        ("POLY_ENV".to_string(), "local".to_string()),
        ("APP_VERSION".to_string(), "test-version".to_string()),
        ("SUPPORTED_MARKETS".to_string(), "btc5m,eth15m".to_string()),
    ])
    .expect("test config should be valid");
    let (bus, rx) = RealtimeBus::bounded(8);
    let state_owner = RealtimeStateOwner::spawn(rx, Default::default(), Vec::new());
    let runtime = RealtimeRuntime::new(bus.clone(), state_owner);

    bus.try_publish(RealtimeEvent::Tick(MarketTick {
        market_key: MarketKey::Btc5m,
        symbol: "BTC-USD".to_string(),
        source: "coinbase".to_string(),
        source_ts: OffsetDateTime::UNIX_EPOCH + time::Duration::seconds(10),
        received_at: OffsetDateTime::UNIX_EPOCH + time::Duration::seconds(10),
        price: "101.25".parse().unwrap(),
        size: Some("0.5".parse().unwrap()),
        sequence: Some(10),
    }))
    .expect("tick publish should succeed");
    tokio::time::sleep(Duration::from_millis(20)).await;

    build_router_with_runtime(config, None, Some(runtime))
}

async fn build_test_app_with_repository(repository: ApiRepository) -> axum::Router {
    let config = AppConfig::from_env_map([
        ("POLY_ENV".to_string(), "local".to_string()),
        ("APP_VERSION".to_string(), "test-version".to_string()),
        ("SUPPORTED_MARKETS".to_string(), "btc5m,eth15m".to_string()),
    ])
    .expect("test config should be valid");

    build_router_with_runtime_and_storage(config, None, None, Some(Arc::new(repository)))
}

#[derive(Clone)]
struct ApiRepository {
    candles: Vec<CandleRecord>,
    signals: Vec<SignalWithMarketRecord>,
}

#[async_trait::async_trait]
impl StorageRepository for ApiRepository {
    async fn insert_raw_market_event(
        &self,
        _event: &NewRawMarketEvent,
    ) -> Result<RawMarketEventRecord, StorageError> {
        unreachable!("api route test does not insert raw events")
    }

    async fn insert_tick(&self, _tick: &NewTick) -> Result<TickRecord, StorageError> {
        unreachable!("api route test does not insert ticks")
    }

    async fn insert_signal(&self, _signal: &NewSignal) -> Result<SignalRecord, StorageError> {
        unreachable!("api route test does not insert signals")
    }

    async fn insert_notification_delivery(
        &self,
        _delivery: &NewNotificationDelivery,
    ) -> Result<Uuid, StorageError> {
        unreachable!("api route test does not insert notifications")
    }

    async fn insert_runtime_event(
        &self,
        _event: &NewRuntimeEvent,
    ) -> Result<RuntimeEventRecord, StorageError> {
        unreachable!("api route test does not insert runtime events")
    }

    async fn replay_ticks_for_window(
        &self,
        _market_key: &str,
        _window_start: OffsetDateTime,
    ) -> Result<Vec<ReplayTick>, StorageError> {
        unreachable!("api route test does not replay ticks")
    }

    async fn recent_candles(
        &self,
        market_key: &str,
        limit: i64,
    ) -> Result<Vec<CandleRecord>, StorageError> {
        Ok(self
            .candles
            .iter()
            .filter(|candle| candle.market_key == market_key)
            .take(limit as usize)
            .cloned()
            .collect())
    }

    async fn latest_signals(
        &self,
        market_key: &str,
        limit: i64,
    ) -> Result<Vec<SignalWithMarketRecord>, StorageError> {
        Ok(self
            .signals
            .iter()
            .filter(|signal| signal.market_key == market_key)
            .take(limit as usize)
            .cloned()
            .collect())
    }
}
