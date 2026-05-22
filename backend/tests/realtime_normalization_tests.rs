use bigdecimal::BigDecimal;
use polymarket_backend::realtime::{
    normalize_coinbase_ticker, normalize_polymarket_snapshot, MarketKey, RealtimeError,
};
use serde_json::json;
use std::str::FromStr;
use time::{Duration, OffsetDateTime};

#[test]
fn coinbase_ticker_json_becomes_market_tick() {
    let received_at = OffsetDateTime::UNIX_EPOCH + Duration::seconds(1_779_330_123);
    let payload = json!({
        "type": "ticker",
        "product_id": "BTC-USD",
        "time": "2026-05-21T10:15:23.456Z",
        "price": "104250.12",
        "last_size": "0.031",
        "sequence": 998877
    });

    let tick = normalize_coinbase_ticker(&payload, received_at).expect("ticker should normalize");

    assert_eq!(tick.market_key, MarketKey::Btc5m);
    assert_eq!(tick.symbol, "BTC-USD");
    assert_eq!(tick.source, "coinbase");
    assert_eq!(tick.received_at, received_at);
    assert_eq!(tick.price, BigDecimal::from_str("104250.12").unwrap());
    assert_eq!(tick.size, Some(BigDecimal::from_str("0.031").unwrap()));
    assert_eq!(tick.sequence, Some(998877));
}

#[test]
fn coinbase_ticker_rejects_unsupported_product() {
    let payload = json!({
        "type": "ticker",
        "product_id": "SOL-USD",
        "time": "2026-05-21T10:15:23.456Z",
        "price": "180.12"
    });

    let error = normalize_coinbase_ticker(&payload, OffsetDateTime::UNIX_EPOCH)
        .expect_err("unsupported product should be rejected");

    assert!(matches!(error, RealtimeError::UnsupportedProduct(product) if product == "SOL-USD"));
}

#[test]
fn polymarket_snapshot_json_becomes_typed_snapshot() {
    let captured_at = OffsetDateTime::UNIX_EPOCH + Duration::seconds(1_779_330_300);
    let payload = json!({
        "event_slug": "btc-updown-5m-1779330300",
        "up_price": "0.515",
        "down_price": "0.485",
        "liquidity": "12500.75"
    });

    let snapshot = normalize_polymarket_snapshot(MarketKey::Btc5m, &payload, captured_at)
        .expect("snapshot should normalize");

    assert_eq!(snapshot.market_key, MarketKey::Btc5m);
    assert_eq!(snapshot.event_slug, "btc-updown-5m-1779330300");
    assert_eq!(snapshot.captured_at, captured_at);
    assert_eq!(
        snapshot.up_price,
        Some(BigDecimal::from_str("0.515").unwrap())
    );
    assert_eq!(
        snapshot.down_price,
        Some(BigDecimal::from_str("0.485").unwrap())
    );
    assert_eq!(
        snapshot.spread,
        Some(BigDecimal::from_str("0.030").unwrap())
    );
    assert_eq!(
        snapshot.liquidity,
        Some(BigDecimal::from_str("12500.75").unwrap())
    );
}

#[test]
fn polymarket_snapshot_uses_explicit_spread_when_present() {
    let captured_at = OffsetDateTime::UNIX_EPOCH + Duration::seconds(1_779_330_300);
    let payload = json!({
        "event_slug": "btc-updown-5m-1779330300",
        "up_price": "0.87",
        "down_price": "0.12",
        "spread": "0.01"
    });

    let snapshot = normalize_polymarket_snapshot(MarketKey::Btc5m, &payload, captured_at)
        .expect("snapshot should normalize");

    assert_eq!(snapshot.spread, Some(BigDecimal::from_str("0.01").unwrap()));
}
