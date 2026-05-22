use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header::CONTENT_TYPE},
};
use polymarket_backend::{
    config::AppConfig,
    notification::{
        NotificationError, NotificationRuntime, NotificationRuntimeConfig, NotificationSendOutcome,
        NotificationSender,
    },
    realtime::{
        MarketKey, MarketTick, RealtimeBus, RealtimeEvent, RealtimeRuntime, RealtimeStateOwner,
    },
    router::{build_router_with_runtime, build_router_with_runtime_and_storage},
    storage::{
        BacktestRunRecord, CandleRecord, ModelAssignmentRecord, NewBacktestRun, NewModelAssignment,
        NewNotificationChannel, NewNotificationDelivery, NewRawMarketEvent, NewRuntimeEvent,
        NewSignal, NewTick, NotificationChannelRecord, NotificationDeliveryRecord,
        RawMarketEventRecord, ReplayTick, RuntimeEventRecord, SignalRecord, SignalWithMarketRecord,
        StorageError, StorageRepository, TickRecord,
    },
};
use serde_json::{Value, json};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
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
        ..Default::default()
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
        ..Default::default()
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

#[tokio::test]
async fn model_assignments_api_lists_active_assignments() {
    let repository = ApiRepository::default();
    repository
        .assignments
        .lock()
        .expect("assignments lock")
        .push(model_assignment("btc5m", "baseline", "0.1.0"));
    let app = build_test_app_with_repository(repository).await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/config/model-assignments")
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

    assert_eq!(
        json["assignments"].as_array().expect("assignments").len(),
        1
    );
    assert_eq!(json["assignments"][0]["market_key"], "btc5m");
    assert_eq!(json["assignments"][0]["model_key"], "baseline");
    assert_eq!(
        json["assignments"][0]["parameters"],
        json!({"threshold_bps": 4})
    );
}

#[tokio::test]
async fn model_assignment_api_sets_active_assignment() {
    let repository = ApiRepository::default();
    let app = build_test_app_with_repository(repository.clone()).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/config/model-assignments/btc5m")
                .header(CONTENT_TYPE, "application/json")
                .header("authorization", "Bearer test-admin-token")
                .body(Body::from(
                    json!({
                        "model_key": "mean-reversion",
                        "display_name": "Mean Reversion",
                        "version": "0.2.0",
                        "parameters": {"threshold_bps": 7}
                    })
                    .to_string(),
                ))
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
    assert_eq!(json["model_key"], "mean-reversion");
    assert_eq!(json["version"], "0.2.0");
    assert_eq!(json["parameters"], json!({"threshold_bps": 7}));
    assert_eq!(
        repository
            .assignments
            .lock()
            .expect("assignments lock")
            .len(),
        1
    );
}

#[tokio::test]
async fn model_assignment_api_rejects_missing_admin_token() {
    let repository = ApiRepository::default();
    let app = build_test_app_with_repository(repository).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/config/model-assignments/btc5m")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "model_key": "mean-reversion",
                        "version": "0.2.0",
                        "parameters": {}
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("request should be handled");

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn notification_channels_api_lists_channels_with_masked_webhooks() {
    let repository = ApiRepository::default();
    repository
        .channels
        .lock()
        .expect("channels lock")
        .push(notification_channel("btc5m", "primary", true));
    let app = build_test_app_with_repository(repository).await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/config/notification-channels?market_key=btc5m")
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
    assert_eq!(json["channels"].as_array().expect("channels").len(), 1);
    assert_eq!(json["channels"][0]["name"], "primary");
    assert_eq!(json["channels"][0]["webhook_url"], Value::Null);
    assert_eq!(
        json["channels"][0]["webhook_url_masked"],
        "https://open.feishu.cn/.../abcd"
    );
}

#[tokio::test]
async fn notification_channels_api_upserts_feishu_channel() {
    let repository = ApiRepository::default();
    let app = build_test_app_with_repository(repository.clone()).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/config/notification-channels")
                .header(CONTENT_TYPE, "application/json")
                .header("authorization", "Bearer test-admin-token")
                .body(Body::from(
                    json!({
                        "market_key": "btc5m",
                        "channel_type": "feishu",
                        "name": "primary",
                        "webhook_url": "https://open.feishu.cn/open-apis/bot/v2/hook/newabcd",
                        "enabled": true
                    })
                    .to_string(),
                ))
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
    assert_eq!(json["name"], "primary");
    assert_eq!(json["webhook_url"], Value::Null);
    assert_eq!(
        json["webhook_url_masked"],
        "https://open.feishu.cn/.../abcd"
    );
    assert_eq!(
        repository.channels.lock().expect("channels lock")[0].webhook_url,
        "https://open.feishu.cn/open-apis/bot/v2/hook/newabcd"
    );
}

#[tokio::test]
async fn feishu_dry_run_api_requires_admin_token() {
    let repository = ApiRepository::default();
    repository
        .channels
        .lock()
        .expect("channels lock")
        .push(notification_channel("btc5m", "primary", true));
    let app = build_test_app_with_repository(repository).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/notifications/feishu/dry-run")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(json!({"market_key": "btc5m"}).to_string()))
                .expect("request should build"),
        )
        .await
        .expect("request should be handled");

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn feishu_dry_run_api_sends_enabled_channels_and_masks_webhooks() {
    let repository = ApiRepository::default();
    repository.channels.lock().expect("channels lock").extend([
        notification_channel("btc5m", "primary", true),
        notification_channel("btc5m", "disabled", false),
        notification_channel("eth15m", "other-market", true),
    ]);
    let sender = Arc::new(ApiFakeSender::default());
    let app = build_test_app_with_repository_and_notification(repository, sender.clone()).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/notifications/feishu/dry-run")
                .header(CONTENT_TYPE, "application/json")
                .header("authorization", "Bearer test-admin-token")
                .body(Body::from(json!({"market_key": "btc5m"}).to_string()))
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
    assert_eq!(json["sent"].as_array().expect("sent").len(), 1);
    assert_eq!(json["sent"][0]["channel"]["name"], "primary");
    assert_eq!(json["sent"][0]["channel"]["webhook_url"], Value::Null);
    assert_eq!(
        json["sent"][0]["channel"]["webhook_url_masked"],
        "https://open.feishu.cn/.../abcd"
    );
    assert_eq!(json["sent"][0]["status"], "sent");
    assert_eq!(json["sent"][0]["response_summary"], "ok");
    assert_eq!(json["card_summary"]["title"], "Polymarket BTC 5m Dry Run");
    assert!(!json.to_string().contains("open-apis/bot/v2/hook/abcd"));
    assert_eq!(sender.requests().len(), 1);
}

#[tokio::test]
async fn runtime_health_api_returns_notification_runtime_snapshot() {
    let repository = ApiRepository::default();
    let sender = Arc::new(ApiFakeSender::default());
    let app = build_test_app_with_repository_and_notification(repository, sender).await;

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

    assert_eq!(json["notification"]["accepted"], 0);
    assert_eq!(json["notification"]["sent"], 0);
    assert_eq!(json["notification"]["failed"], 0);
}

#[tokio::test]
async fn backtests_api_returns_latest_runs_for_market_and_model() {
    let app = build_test_app_with_repository(ApiRepository {
        backtest_runs: vec![backtest_run("btc5m", "baseline_direction", "0.1.0")],
        ..Default::default()
    })
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/backtests?market_key=btc5m&model_key=baseline_direction&limit=5")
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

    assert_eq!(json["runs"].as_array().expect("runs").len(), 1);
    assert_eq!(json["runs"][0]["market_key"], "btc5m");
    assert_eq!(json["runs"][0]["model_key"], "baseline_direction");
    assert_eq!(json["runs"][0]["metrics"]["eligible"], false);
}

#[tokio::test]
async fn cockpit_bootstrap_api_returns_frontend_contract_without_secrets() {
    let signal = signal_record("btc5m", "Up", "positive edge");
    let channel = notification_channel("btc5m", "primary", true);
    let repository = ApiRepository {
        candles: vec![CandleRecord {
            market_key: "btc5m".to_string(),
            start_ts: OffsetDateTime::UNIX_EPOCH + time::Duration::seconds(60),
            open: "100.0".parse().unwrap(),
            high: "103.0".parse().unwrap(),
            low: "99.0".parse().unwrap(),
            close: "102.5".parse().unwrap(),
            volume: "4.2".parse().unwrap(),
        }],
        signals: vec![signal.clone()],
        backtest_runs: vec![backtest_run("btc5m", "baseline_direction", "0.1.0")],
        deliveries: vec![notification_delivery("btc5m", signal.id, &channel, "sent")],
        ..Default::default()
    };
    repository
        .assignments
        .lock()
        .expect("assignments lock")
        .push(model_assignment("btc5m", "baseline", "0.1.0"));
    repository
        .channels
        .lock()
        .expect("channels lock")
        .push(channel);
    let app = build_test_app_with_repository(repository).await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/cockpit/bootstrap")
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

    assert_eq!(json["markets"].as_array().expect("markets").len(), 2);
    let btc = json["markets"]
        .as_array()
        .expect("markets")
        .iter()
        .find(|market| market["summary"]["market_key"] == "btc5m")
        .expect("btc market should be present");
    assert_eq!(btc["summary"]["symbol"], "BTC-USD");
    assert_eq!(btc["recent_candles"].as_array().expect("candles").len(), 1);
    assert_eq!(btc["active_model"]["model_key"], "baseline");
    assert_eq!(btc["latest_signal"]["side"], "Up");
    assert_eq!(btc["latest_actionable_alert"]["side"], "Up");
    assert_eq!(btc["latest_backtest"]["model_key"], "baseline_direction");
    assert_eq!(btc["notification_channels"][0]["webhook_url"], Value::Null);
    assert_eq!(btc["notification_deliveries"][0]["status"], "sent");
    assert!(json["runtime"].is_object());
    assert!(!json.to_string().contains("open-apis/bot/v2/hook/abcd"));
}

#[tokio::test]
async fn notification_deliveries_api_returns_masked_status_for_market() {
    let signal = signal_record("btc5m", "Down", "delivery status");
    let channel = notification_channel("btc5m", "primary", true);
    let mut delivery = notification_delivery("btc5m", signal.id, &channel, "failed");
    delivery.response_summary =
        Some("failed calling https://open.feishu.cn/open-apis/bot/v2/hook/abcd".to_string());
    let app = build_test_app_with_repository(ApiRepository {
        deliveries: vec![delivery],
        ..Default::default()
    })
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/notifications/deliveries?market_key=btc5m&limit=20")
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
    assert_eq!(json["deliveries"].as_array().expect("deliveries").len(), 1);
    assert_eq!(json["deliveries"][0]["channel_name"], "primary");
    assert_eq!(json["deliveries"][0]["channel_type"], "feishu");
    assert_eq!(json["deliveries"][0]["status"], "failed");
    assert_eq!(json["deliveries"][0]["attempt_count"], 1);
    assert_eq!(
        json["deliveries"][0]["response_summary"],
        "failed calling https://open.feishu.cn/open-apis/bot/v2/hook/****"
    );
    assert!(!json.to_string().contains("open-apis/bot/v2/hook/abcd"));
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
        (
            "ADMIN_API_TOKEN".to_string(),
            "test-admin-token".to_string(),
        ),
    ])
    .expect("test config should be valid");

    build_router_with_runtime_and_storage(config, None, None, Some(Arc::new(repository)))
}

async fn build_test_app_with_repository_and_notification(
    repository: ApiRepository,
    sender: Arc<ApiFakeSender>,
) -> axum::Router {
    let config = AppConfig::from_env_map([
        ("POLY_ENV".to_string(), "local".to_string()),
        ("APP_VERSION".to_string(), "test-version".to_string()),
        ("SUPPORTED_MARKETS".to_string(), "btc5m,eth15m".to_string()),
        (
            "ADMIN_API_TOKEN".to_string(),
            "test-admin-token".to_string(),
        ),
    ])
    .expect("test config should be valid");
    let (writer, writer_task) = polymarket_backend::storage::StorageWriter::spawn(
        Arc::new(repository.clone()),
        8,
        Duration::from_millis(5),
    );
    let notification = NotificationRuntime::spawn_paused_for_tests(
        sender,
        writer,
        8,
        NotificationRuntimeConfig {
            max_attempts: 1,
            send_timeout_ms: 200,
            retry_base_delay_ms: 1,
        },
    );
    writer_task.abort();

    polymarket_backend::router::build_router_with_runtime_storage_and_notification(
        config,
        None,
        None,
        Some(Arc::new(repository)),
        Some(notification),
    )
}

#[derive(Clone, Default)]
struct ApiRepository {
    candles: Vec<CandleRecord>,
    signals: Vec<SignalWithMarketRecord>,
    backtest_runs: Vec<BacktestRunRecord>,
    deliveries: Vec<NotificationDeliveryRecord>,
    assignments: Arc<Mutex<Vec<ModelAssignmentRecord>>>,
    channels: Arc<Mutex<Vec<NotificationChannelRecord>>>,
}

#[derive(Default)]
struct ApiFakeSender {
    requests: Mutex<Vec<Value>>,
}

impl ApiFakeSender {
    fn requests(&self) -> Vec<Value> {
        self.requests.lock().expect("requests lock").clone()
    }
}

#[async_trait::async_trait]
impl NotificationSender for ApiFakeSender {
    async fn send(
        &self,
        _webhook_url: &str,
        payload: Value,
    ) -> Result<NotificationSendOutcome, NotificationError> {
        self.requests.lock().expect("requests lock").push(payload);
        Ok(NotificationSendOutcome {
            response_summary: Some("ok".to_string()),
        })
    }
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

    async fn latest_notification_deliveries(
        &self,
        market_key: &str,
        limit: i64,
    ) -> Result<Vec<NotificationDeliveryRecord>, StorageError> {
        Ok(self
            .deliveries
            .iter()
            .filter(|delivery| delivery.market_key == market_key)
            .take(limit.clamp(1, 200) as usize)
            .cloned()
            .collect())
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

    async fn list_model_assignments(&self) -> Result<Vec<ModelAssignmentRecord>, StorageError> {
        Ok(self.assignments.lock().expect("assignments lock").clone())
    }

    async fn set_active_model_assignment(
        &self,
        assignment: &NewModelAssignment,
    ) -> Result<ModelAssignmentRecord, StorageError> {
        let record = ModelAssignmentRecord {
            market_key: assignment.market_key.clone(),
            model_key: assignment.model_key.clone(),
            display_name: assignment.display_name.clone(),
            version: assignment.version.clone(),
            parameters: assignment.parameters.clone(),
            status: "active".to_string(),
            created_at: OffsetDateTime::UNIX_EPOCH,
        };
        let mut assignments = self.assignments.lock().expect("assignments lock");
        assignments.retain(|existing| existing.market_key != assignment.market_key);
        assignments.push(record.clone());
        Ok(record)
    }

    async fn list_notification_channels(
        &self,
        market_key: &str,
    ) -> Result<Vec<NotificationChannelRecord>, StorageError> {
        Ok(self
            .channels
            .lock()
            .expect("channels lock")
            .iter()
            .filter(|channel| channel.market_key == market_key)
            .cloned()
            .collect())
    }

    async fn upsert_notification_channel(
        &self,
        channel: &NewNotificationChannel,
    ) -> Result<NotificationChannelRecord, StorageError> {
        let record = NotificationChannelRecord {
            id: Uuid::new_v4(),
            market_key: channel.market_key.clone(),
            channel_type: channel.channel_type.clone(),
            name: channel.name.clone(),
            webhook_url: channel.webhook_url.clone(),
            enabled: channel.enabled,
            created_at: OffsetDateTime::UNIX_EPOCH,
        };
        let mut channels = self.channels.lock().expect("channels lock");
        channels.retain(|existing| {
            !(existing.market_key == channel.market_key
                && existing.channel_type == channel.channel_type
                && existing.name == channel.name)
        });
        channels.push(record.clone());
        Ok(record)
    }

    async fn insert_backtest_run(
        &self,
        _run: &NewBacktestRun,
    ) -> Result<BacktestRunRecord, StorageError> {
        unreachable!("api route test does not insert backtest runs")
    }

    async fn latest_backtest_runs(
        &self,
        market_key: Option<&str>,
        model_key: Option<&str>,
        limit: i64,
    ) -> Result<Vec<BacktestRunRecord>, StorageError> {
        Ok(self
            .backtest_runs
            .iter()
            .filter(|run| market_key.is_none_or(|value| run.market_key == value))
            .filter(|run| model_key.is_none_or(|value| run.model_key == value))
            .take(limit.clamp(1, 100) as usize)
            .cloned()
            .collect())
    }
}

fn model_assignment(market_key: &str, model_key: &str, version: &str) -> ModelAssignmentRecord {
    ModelAssignmentRecord {
        market_key: market_key.to_string(),
        model_key: model_key.to_string(),
        display_name: "Baseline".to_string(),
        version: version.to_string(),
        parameters: json!({"threshold_bps": 4}),
        status: "active".to_string(),
        created_at: OffsetDateTime::UNIX_EPOCH,
    }
}

fn notification_channel(market_key: &str, name: &str, enabled: bool) -> NotificationChannelRecord {
    NotificationChannelRecord {
        id: Uuid::new_v4(),
        market_key: market_key.to_string(),
        channel_type: "feishu".to_string(),
        name: name.to_string(),
        webhook_url: "https://open.feishu.cn/open-apis/bot/v2/hook/abcd".to_string(),
        enabled,
        created_at: OffsetDateTime::UNIX_EPOCH,
    }
}

fn notification_delivery(
    market_key: &str,
    signal_id: Uuid,
    channel: &NotificationChannelRecord,
    status: &str,
) -> NotificationDeliveryRecord {
    NotificationDeliveryRecord {
        id: Uuid::new_v4(),
        market_key: market_key.to_string(),
        signal_id,
        channel_id: channel.id,
        channel_type: channel.channel_type.clone(),
        channel_name: channel.name.clone(),
        status: status.to_string(),
        attempt_count: 1,
        response_summary: Some("ok".to_string()),
        created_at: OffsetDateTime::UNIX_EPOCH + time::Duration::seconds(130),
        updated_at: OffsetDateTime::UNIX_EPOCH + time::Duration::seconds(131),
    }
}

fn signal_record(market_key: &str, side: &str, reason: &str) -> SignalWithMarketRecord {
    SignalWithMarketRecord {
        id: Uuid::new_v4(),
        market_key: market_key.to_string(),
        market_window_id: Uuid::new_v4(),
        model_version_id: Uuid::new_v4(),
        signal_type: "actionable_alert".to_string(),
        side: Some(side.to_string()),
        confidence: Some("0.82".parse().unwrap()),
        limit_price: Some("0.51".parse().unwrap()),
        suggested_size: Some("2.5".parse().unwrap()),
        ttl_ms: Some(15_000),
        reason: reason.to_string(),
        features: serde_json::json!({"edge_bps": 6}),
        input_snapshot_hash: format!("snapshot-{market_key}-{side}"),
        created_at: OffsetDateTime::UNIX_EPOCH + time::Duration::seconds(90),
    }
}

fn backtest_run(market_key: &str, model_key: &str, model_version: &str) -> BacktestRunRecord {
    BacktestRunRecord {
        id: Uuid::new_v4(),
        market_key: market_key.to_string(),
        model_key: model_key.to_string(),
        display_name: "Baseline".to_string(),
        model_version: model_version.to_string(),
        parameters: json!({"threshold_bps": 4}),
        started_at: OffsetDateTime::UNIX_EPOCH + time::Duration::seconds(120),
        finished_at: Some(OffsetDateTime::UNIX_EPOCH + time::Duration::seconds(121)),
        window_start: OffsetDateTime::UNIX_EPOCH,
        window_end: OffsetDateTime::UNIX_EPOCH + time::Duration::minutes(5),
        metrics: json!({
            "trades": 12,
            "wins": 7,
            "wilson_lower_bound": "0.32",
            "eligible": false
        }),
        status: "completed".to_string(),
    }
}
