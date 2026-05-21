use bigdecimal::BigDecimal;
use polymarket_backend::realtime::{window_for_tick, CandleAggregator, MarketKey, MarketTick};
use std::str::FromStr;
use time::{Duration, OffsetDateTime};

#[test]
fn btc_5m_tick_maps_to_five_minute_market_window() {
    let tick_ts = OffsetDateTime::UNIX_EPOCH + Duration::seconds(1_779_330_456);

    let window = window_for_tick(MarketKey::Btc5m, tick_ts);

    assert_eq!(
        window.start_ts,
        OffsetDateTime::UNIX_EPOCH + Duration::seconds(1_779_330_300)
    );
    assert_eq!(
        window.end_ts,
        OffsetDateTime::UNIX_EPOCH + Duration::seconds(1_779_330_600)
    );
    assert_eq!(window.event_slug, "btc-updown-5m-1779330300");
}

#[test]
fn eth_15m_tick_maps_to_fifteen_minute_market_window() {
    let tick_ts = OffsetDateTime::UNIX_EPOCH + Duration::seconds(1_779_330_456);

    let window = window_for_tick(MarketKey::Eth15m, tick_ts);

    assert_eq!(
        window.start_ts,
        OffsetDateTime::UNIX_EPOCH + Duration::seconds(1_779_329_700)
    );
    assert_eq!(
        window.end_ts,
        OffsetDateTime::UNIX_EPOCH + Duration::seconds(1_779_330_600)
    );
    assert_eq!(window.event_slug, "eth-updown-15m-1779329700");
}

#[test]
fn candle_aggregator_tracks_ohlcv_for_one_minute_bucket() {
    let mut aggregator = CandleAggregator::new();
    let first_ts = OffsetDateTime::UNIX_EPOCH + Duration::seconds(1_779_330_305);
    let second_ts = OffsetDateTime::UNIX_EPOCH + Duration::seconds(1_779_330_320);
    let third_ts = OffsetDateTime::UNIX_EPOCH + Duration::seconds(1_779_330_352);

    aggregator.apply_tick(&tick_at(first_ts, "100.0", "0.5"));
    aggregator.apply_tick(&tick_at(second_ts, "105.5", "0.2"));
    aggregator.apply_tick(&tick_at(third_ts, "99.5", "0.3"));

    let candles = aggregator.snapshots();

    assert_eq!(candles.len(), 1);
    assert_eq!(
        candles[0].start_ts,
        OffsetDateTime::UNIX_EPOCH + Duration::seconds(1_779_330_300)
    );
    assert_eq!(candles[0].open, BigDecimal::from_str("100.0").unwrap());
    assert_eq!(candles[0].high, BigDecimal::from_str("105.5").unwrap());
    assert_eq!(candles[0].low, BigDecimal::from_str("99.5").unwrap());
    assert_eq!(candles[0].close, BigDecimal::from_str("99.5").unwrap());
    assert_eq!(candles[0].volume, BigDecimal::from_str("1.0").unwrap());
}

fn tick_at(source_ts: OffsetDateTime, price: &str, size: &str) -> MarketTick {
    MarketTick {
        market_key: MarketKey::Btc5m,
        symbol: "BTC-USD".to_string(),
        source: "coinbase".to_string(),
        source_ts,
        received_at: source_ts + Duration::seconds(1),
        price: BigDecimal::from_str(price).unwrap(),
        size: Some(BigDecimal::from_str(size).unwrap()),
        sequence: None,
    }
}
