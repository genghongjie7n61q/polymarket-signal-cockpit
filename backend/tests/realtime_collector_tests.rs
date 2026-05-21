use polymarket_backend::realtime::{
    build_coinbase_subscribe_message, handle_coinbase_ws_message, CoinbaseCollectorConfig,
    CollectorEvent, CollectorEventSink, CollectorIngress, MarketKey,
    PolymarketSnapshotRefresherConfig, RealtimeEvent, COINBASE_WS_ENDPOINT,
};
use serde_json::json;
use std::sync::{Arc, Mutex};
use time::{Duration, OffsetDateTime};

#[test]
fn collector_config_defaults_to_supported_symbols_and_markets() {
    let coinbase = CoinbaseCollectorConfig::default();
    let polymarket = PolymarketSnapshotRefresherConfig::default();

    assert_eq!(coinbase.symbols, vec!["BTC-USD", "ETH-USD"]);
    assert_eq!(coinbase.connect_timeout, std::time::Duration::from_secs(10));
    assert_eq!(
        polymarket.markets,
        vec![MarketKey::Btc5m, MarketKey::Eth15m]
    );
    assert_eq!(polymarket.interval, Duration::seconds(15));
}

#[test]
fn collector_ingress_normalizes_coinbase_ticker_and_publishes_tick() {
    let sink = RecordingSink::default();
    let ingress = CollectorIngress::new(sink.clone());
    let received_at = OffsetDateTime::UNIX_EPOCH + Duration::seconds(10);

    ingress
        .handle(CollectorEvent::CoinbaseTicker {
            payload: json!({
                "type": "ticker",
                "product_id": "BTC-USD",
                "time": "2026-05-21T12:00:01Z",
                "price": "68000.25",
                "last_size": "0.005",
                "sequence": 42
            }),
            received_at,
        })
        .expect("ticker should be published");

    let events = sink.events();
    assert_eq!(events.len(), 1);
    let RealtimeEvent::Tick(tick) = &events[0] else {
        panic!("expected tick event");
    };
    assert_eq!(tick.market_key, MarketKey::Btc5m);
    assert_eq!(tick.source, "coinbase");
    assert_eq!(tick.received_at, received_at);
}

#[test]
fn collector_ingress_normalizes_polymarket_snapshot_and_heartbeat() {
    let sink = RecordingSink::default();
    let ingress = CollectorIngress::new(sink.clone());
    let captured_at = OffsetDateTime::UNIX_EPOCH + Duration::seconds(20);

    ingress
        .handle(CollectorEvent::PolymarketSnapshot {
            market_key: MarketKey::Eth15m,
            payload: json!({
                "event_slug": "eth-updown-15m-1779324300",
                "up_price": "0.54",
                "down_price": "0.47",
                "liquidity": "1000.0"
            }),
            captured_at,
        })
        .expect("snapshot should be published");
    ingress
        .handle(CollectorEvent::Heartbeat {
            source: "coinbase".to_string(),
            received_at: captured_at,
        })
        .expect("heartbeat should be published");

    let events = sink.events();
    assert_eq!(events.len(), 2);
    assert!(matches!(
        &events[0],
        RealtimeEvent::PolymarketSnapshot(snapshot)
            if snapshot.market_key == MarketKey::Eth15m
                && snapshot.event_slug == "eth-updown-15m-1779324300"
    ));
    assert!(matches!(
        &events[1],
        RealtimeEvent::SourceHeartbeat { source, received_at }
            if source == "coinbase" && *received_at == captured_at
    ));
}

#[test]
fn coinbase_subscribe_message_uses_public_ticker_and_heartbeat_channels() {
    let config = CoinbaseCollectorConfig::default();
    let message = build_coinbase_subscribe_message(&config);

    assert_eq!(config.endpoint, COINBASE_WS_ENDPOINT);
    assert_eq!(message["type"], "subscribe");
    assert_eq!(message["product_ids"], json!(["BTC-USD", "ETH-USD"]));
    assert_eq!(message["channel"], "ticker");
}

#[test]
fn coinbase_ws_handler_publishes_ticker_and_heartbeat_messages() {
    let sink = RecordingSink::default();
    let ingress = CollectorIngress::new(sink.clone());
    let received_at = OffsetDateTime::UNIX_EPOCH + Duration::seconds(30);

    handle_coinbase_ws_message(
        &ingress,
        &json!({
            "channel": "ticker",
            "timestamp": "2026-05-21T12:00:02Z",
            "sequence_num": 43,
            "events": [{
                "type": "snapshot",
                "tickers": [{
                    "type": "ticker",
                    "product_id": "ETH-USD",
                    "price": "3600.25"
                }]
            }]
        }),
        received_at,
    )
    .expect("ticker should publish");
    handle_coinbase_ws_message(
        &ingress,
        &json!({
            "channel": "heartbeats",
            "timestamp": "2026-05-21T12:00:03Z",
            "events": [{
                "current_time": "2026-05-21T12:00:03Z"
            }]
        }),
        received_at,
    )
    .expect("heartbeat should publish");

    let events = sink.events();
    assert_eq!(events.len(), 2);
    assert!(matches!(
        &events[0],
        RealtimeEvent::Tick(tick) if tick.market_key == MarketKey::Eth15m
    ));
    assert!(matches!(
        &events[1],
        RealtimeEvent::SourceHeartbeat { source, received_at: ts }
            if source == "coinbase" && *ts == received_at
    ));
}

#[derive(Clone, Default)]
struct RecordingSink {
    events: Arc<Mutex<Vec<RealtimeEvent>>>,
}

impl RecordingSink {
    fn events(&self) -> Vec<RealtimeEvent> {
        self.events.lock().expect("events lock poisoned").clone()
    }
}

impl CollectorEventSink for RecordingSink {
    fn try_publish(
        &self,
        event: RealtimeEvent,
    ) -> Result<(), polymarket_backend::realtime::RealtimeError> {
        self.events
            .lock()
            .expect("events lock poisoned")
            .push(event);
        Ok(())
    }
}
