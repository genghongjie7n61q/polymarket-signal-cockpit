use polymarket_backend::realtime::{
    build_coinbase_subscribe_message, build_polymarket_prices_request, extract_polymarket_market,
    handle_coinbase_ws_message, merge_polymarket_snapshot_payload, polymarket_event_slug_at,
    CoinbaseCollectorConfig, CollectorEvent, CollectorEventSink, CollectorIngress, MarketKey,
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

#[test]
fn polymarket_event_slug_uses_supported_window_boundaries() {
    let ts = OffsetDateTime::from_unix_timestamp(1_779_414_219).expect("valid ts");

    assert_eq!(
        polymarket_event_slug_at(MarketKey::Btc5m, ts),
        "btc-updown-5m-1779414000"
    );
    assert_eq!(
        polymarket_event_slug_at(MarketKey::Eth15m, ts),
        "eth-updown-15m-1779413400"
    );
}

#[test]
fn polymarket_gamma_event_extracts_tokens_and_fallback_prices() {
    let event = json!({
        "slug": "btc-updown-5m-1779411300",
        "markets": [{
            "slug": "btc-updown-5m-1779411300",
            "outcomes": "[\"Up\", \"Down\"]",
            "outcomePrices": "[\"0.505\", \"0.495\"]",
            "clobTokenIds": "[\"up-token\", \"down-token\"]",
            "liquidity": "12272.3924",
            "spread": 0.01,
            "acceptingOrders": true,
            "closed": false
        }]
    });

    let market = extract_polymarket_market(MarketKey::Btc5m, "btc-updown-5m-1779411300", &event)
        .expect("market metadata should parse");

    assert_eq!(market.event_slug, "btc-updown-5m-1779411300");
    assert_eq!(market.up_token_id, "up-token");
    assert_eq!(market.down_token_id, "down-token");
    assert_eq!(market.fallback_up_price.as_deref(), Some("0.505"));
    assert_eq!(market.fallback_down_price.as_deref(), Some("0.495"));
    assert_eq!(market.liquidity.as_deref(), Some("12272.3924"));
}

#[test]
fn polymarket_gamma_event_rejects_missing_requested_market_slug() {
    let event = json!({
        "slug": "btc-updown-5m-1779411300",
        "markets": [{
            "slug": "btc-updown-5m-1779411000",
            "outcomes": "[\"Up\", \"Down\"]",
            "outcomePrices": "[\"0.505\", \"0.495\"]",
            "clobTokenIds": "[\"wrong-up-token\", \"wrong-down-token\"]"
        }]
    });

    let error = extract_polymarket_market(MarketKey::Btc5m, "btc-updown-5m-1779411300", &event)
        .expect_err("wrong market slug should not be used as fallback");

    assert!(error
        .to_string()
        .contains("missing market slug btc-updown-5m-1779411300"));
}

#[test]
fn polymarket_snapshot_payload_prefers_clob_prices_over_gamma_prices() {
    let event = json!({
        "slug": "btc-updown-5m-1779411300",
        "markets": [{
            "slug": "btc-updown-5m-1779411300",
            "outcomes": "[\"Up\", \"Down\"]",
            "outcomePrices": "[\"0.505\", \"0.495\"]",
            "clobTokenIds": "[\"up-token\", \"down-token\"]",
            "liquidity": "12272.3924",
            "spread": 0.01
        }]
    });
    let market = extract_polymarket_market(MarketKey::Btc5m, "btc-updown-5m-1779411300", &event)
        .expect("market metadata should parse");
    let prices = json!({
        "up-token": { "BUY": "0.87" },
        "down-token": { "BUY": "0.12" }
    });

    let payload =
        merge_polymarket_snapshot_payload(&market, &prices).expect("snapshot payload should merge");

    assert_eq!(payload["event_slug"], "btc-updown-5m-1779411300");
    assert_eq!(payload["up_price"], "0.87");
    assert_eq!(payload["down_price"], "0.12");
    assert_eq!(payload["liquidity"], "12272.3924");
    assert_eq!(payload["up_token_id"], "up-token");
    assert_eq!(payload["down_token_id"], "down-token");
}

#[test]
fn polymarket_prices_request_uses_buy_side_for_up_and_down_tokens() {
    let request = build_polymarket_prices_request("up-token", "down-token");

    assert_eq!(
        request,
        json!([
            { "token_id": "up-token", "side": "BUY" },
            { "token_id": "down-token", "side": "BUY" }
        ])
    );
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
