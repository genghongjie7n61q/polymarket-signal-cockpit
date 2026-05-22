use bigdecimal::BigDecimal;
use polymarket_backend::realtime::{
    MarketKey, MarketTick, RealtimeBus, RealtimeError, RealtimeEvent, RealtimeStateOwner,
    StateOwnerConfig,
};
use std::str::FromStr;
use time::{Duration, OffsetDateTime};
use tokio::sync::mpsc;

#[test]
fn realtime_bus_reports_queue_full_without_blocking() {
    let (bus, _rx) = RealtimeBus::bounded(1);

    bus.try_publish(RealtimeEvent::Tick(tick_at(10, "100.0")))
        .expect("first publish should fit");
    let error = bus
        .try_publish(RealtimeEvent::Tick(tick_at(11, "101.0")))
        .expect_err("second publish should hit bounded queue");

    assert!(matches!(error, RealtimeError::QueueFull));
    assert_eq!(bus.snapshot().accepted, 1);
    assert_eq!(bus.snapshot().dropped, 1);
}

#[tokio::test]
async fn state_owner_updates_live_state_from_tick_events() {
    let (bus, rx) = RealtimeBus::bounded(8);
    let owner = RealtimeStateOwner::spawn(rx, StateOwnerConfig::default(), Vec::new());

    bus.try_publish(RealtimeEvent::Tick(tick_at(65, "101.5")))
        .expect("tick publish should succeed");

    wait_until(Duration::seconds(1), || {
        owner.snapshot().markets.get(&MarketKey::Btc5m).is_some()
    })
    .await;
    let snapshot = owner.snapshot();
    let market = snapshot
        .markets
        .get(&MarketKey::Btc5m)
        .expect("btc state should exist");

    assert_eq!(market.latest_tick.as_ref().unwrap().price, decimal("101.5"));
    assert_eq!(
        market.current_window.as_ref().unwrap().event_slug,
        "btc-updown-5m-0"
    );
    assert_eq!(market.candles.len(), 1);
}

#[tokio::test]
async fn state_owner_fanout_does_not_wait_for_downstream_receivers() {
    let (bus, rx) = RealtimeBus::bounded(8);
    let (fanout_tx, mut fanout_rx) = mpsc::channel(1);
    let owner = RealtimeStateOwner::spawn(rx, StateOwnerConfig::default(), vec![fanout_tx]);

    bus.try_publish(RealtimeEvent::Tick(tick_at(1, "100.0")))
        .expect("first tick publish");
    bus.try_publish(RealtimeEvent::Tick(tick_at(2, "101.0")))
        .expect("second tick publish should not block on full fanout");

    let forwarded = fanout_rx
        .recv()
        .await
        .expect("one event should be forwarded");
    assert!(matches!(forwarded, RealtimeEvent::Tick(_)));

    wait_until(Duration::seconds(1), || {
        owner.snapshot().metrics.processed >= 2
    })
    .await;
    let snapshot = owner.snapshot();

    assert_eq!(snapshot.metrics.processed, 2);
    assert_eq!(snapshot.metrics.fanout_dropped, 1);
}

#[tokio::test]
async fn state_owner_reports_stale_sources() {
    let (bus, rx) = RealtimeBus::bounded(8);
    let owner = RealtimeStateOwner::spawn(
        rx,
        StateOwnerConfig {
            stale_after: Duration::seconds(30),
        },
        Vec::new(),
    );
    let heartbeat_at = OffsetDateTime::UNIX_EPOCH + Duration::seconds(100);

    bus.try_publish(RealtimeEvent::SourceHeartbeat {
        source: "coinbase".to_string(),
        received_at: heartbeat_at,
    })
    .expect("heartbeat publish");

    wait_until(Duration::seconds(1), || {
        owner.snapshot().sources.contains_key("coinbase")
    })
    .await;

    let fresh = owner.source_status_at(heartbeat_at + Duration::seconds(10));
    let stale = owner.source_status_at(heartbeat_at + Duration::seconds(31));

    assert_eq!(fresh["coinbase"], "fresh");
    assert_eq!(stale["coinbase"], "stale");
}

async fn wait_until<F>(timeout: Duration, condition: F)
where
    F: Fn() -> bool,
{
    let started_at = tokio::time::Instant::now();
    while started_at.elapsed()
        < std::time::Duration::from_millis(timeout.whole_milliseconds() as u64)
    {
        if condition() {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
}

fn tick_at(seconds: i64, price: &str) -> MarketTick {
    let source_ts = OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds);
    MarketTick {
        market_key: MarketKey::Btc5m,
        symbol: "BTC-USD".to_string(),
        source: "coinbase".to_string(),
        source_ts,
        received_at: source_ts,
        price: decimal(price),
        size: Some(decimal("0.1")),
        sequence: Some(seconds),
    }
}

fn decimal(value: &str) -> BigDecimal {
    BigDecimal::from_str(value).unwrap()
}
