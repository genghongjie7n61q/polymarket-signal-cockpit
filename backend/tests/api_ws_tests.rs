use futures_util::StreamExt;
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
use tokio::net::TcpListener;
use tokio_tungstenite::connect_async;

#[tokio::test]
async fn markets_ws_sends_initial_snapshot() {
    let (app, _bus) = build_test_app_with_runtime().await;
    let url = spawn_test_server(app).await;

    let (mut ws, _) = connect_async(format!("{url}/api/ws/markets"))
        .await
        .expect("websocket should connect");
    let json = read_json_message(&mut ws).await;

    assert_eq!(json["type"], "snapshot");
    assert_eq!(json["markets"][0]["market_key"], "btc5m");
    assert_eq!(json["markets"][0]["latest_tick"]["price"], "101.25");
}

#[tokio::test]
async fn markets_ws_sends_later_snapshot_after_realtime_update() {
    let (app, bus) = build_test_app_with_runtime().await;
    let url = spawn_test_server(app).await;

    let (mut ws, _) = connect_async(format!("{url}/api/ws/markets"))
        .await
        .expect("websocket should connect");
    let _initial = read_json_message(&mut ws).await;

    bus.try_publish(RealtimeEvent::Tick(tick_at(20, "102.25")))
        .expect("tick publish should succeed");

    let updated = read_json_message(&mut ws).await;

    assert_eq!(updated["type"], "snapshot");
    assert_eq!(updated["markets"][0]["latest_tick"]["price"], "102.25");
}

#[tokio::test]
async fn markets_ws_returns_error_payload_for_unsupported_query() {
    let (app, _bus) = build_test_app_with_runtime().await;
    let url = spawn_test_server(app).await;

    let (mut ws, _) = connect_async(format!("{url}/api/ws/markets?unsupported=true"))
        .await
        .expect("websocket should connect");
    let json = read_json_message(&mut ws).await;

    assert_eq!(json["type"], "error");
    assert_eq!(json["error"], "unsupported_query");
}

async fn build_test_app_with_runtime() -> (axum::Router, RealtimeBus) {
    let config = AppConfig::from_env_map([
        ("POLY_ENV".to_string(), "local".to_string()),
        ("APP_VERSION".to_string(), "test-version".to_string()),
        ("SUPPORTED_MARKETS".to_string(), "btc5m,eth15m".to_string()),
    ])
    .expect("test config should be valid");
    let (bus, rx) = RealtimeBus::bounded(8);
    let state_owner = RealtimeStateOwner::spawn(rx, Default::default(), Vec::new());
    let runtime = RealtimeRuntime::new(bus.clone(), state_owner);

    bus.try_publish(RealtimeEvent::Tick(tick_at(10, "101.25")))
        .expect("tick publish should succeed");
    tokio::time::sleep(Duration::from_millis(20)).await;

    (build_router_with_runtime(config, None, Some(runtime)), bus)
}

async fn spawn_test_server(app: axum::Router) -> String {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let addr = listener.local_addr().expect("listener should have address");
    tokio::spawn(async move {
        axum::serve(listener, app)
            .await
            .expect("test server should run");
    });
    format!("ws://{addr}")
}

async fn read_json_message<S>(ws: &mut S) -> Value
where
    S: futures_util::Stream<
            Item = Result<
                tokio_tungstenite::tungstenite::Message,
                tokio_tungstenite::tungstenite::Error,
            >,
        > + Unpin,
{
    let message = tokio::time::timeout(Duration::from_secs(3), ws.next())
        .await
        .expect("websocket should send before timeout")
        .expect("websocket should stay open")
        .expect("message should be readable");
    serde_json::from_str(message.to_text().expect("message should be text"))
        .expect("message should be json")
}

fn tick_at(seconds: i64, price: &str) -> MarketTick {
    MarketTick {
        market_key: MarketKey::Btc5m,
        symbol: "BTC-USD".to_string(),
        source: "coinbase".to_string(),
        source_ts: OffsetDateTime::UNIX_EPOCH + time::Duration::seconds(seconds),
        received_at: OffsetDateTime::UNIX_EPOCH + time::Duration::seconds(seconds),
        price: price.parse().unwrap(),
        size: Some("0.5".parse().unwrap()),
        sequence: Some(seconds),
    }
}
