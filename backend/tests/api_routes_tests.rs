use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use polymarket_backend::{
    config::AppConfig,
    realtime::{
        MarketKey, MarketTick, RealtimeBus, RealtimeEvent, RealtimeRuntime, RealtimeStateOwner,
    },
    router::build_router_with_runtime,
};
use serde_json::Value;
use std::time::Duration;
use time::OffsetDateTime;
use tower::ServiceExt;

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
