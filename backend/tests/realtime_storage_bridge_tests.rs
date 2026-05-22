use bigdecimal::BigDecimal;
use polymarket_backend::{
    realtime::{MarketKey, MarketTick, RealtimeEvent, RealtimeStorageBridge},
    storage::{
        CandleRecord, ModelAssignmentRecord, NewModelAssignment, NewNotificationChannel,
        NewNotificationDelivery, NewRawMarketEvent, NewRuntimeEvent, NewSignal, NewTick,
        NotificationChannelRecord, RawMarketEventRecord, ReplayTick, RuntimeEventRecord,
        SignalRecord, SignalWithMarketRecord, StorageError, StorageRepository, StorageWriter,
        TickRecord,
    },
};
use std::{
    collections::BTreeMap,
    str::FromStr,
    sync::{Arc, Mutex},
    time::Duration as StdDuration,
};
use time::{Duration, OffsetDateTime};
use tokio::sync::mpsc;
use uuid::Uuid;

#[tokio::test]
async fn storage_bridge_enqueues_ticks_without_blocking_realtime_state() {
    let repo = Arc::new(CapturedRepository::default());
    let (writer, writer_task) = StorageWriter::spawn(repo.clone(), 8, StdDuration::from_millis(5));
    let (tx, rx) = mpsc::channel(8);
    let market_id = Uuid::new_v4();
    let bridge = RealtimeStorageBridge::spawn(
        rx,
        writer.clone(),
        BTreeMap::from([(MarketKey::Btc5m, market_id)]),
    );

    tx.try_send(RealtimeEvent::Tick(tick_at(10, "68000.25")))
        .expect("bridge input should accept tick");

    wait_until(StdDuration::from_secs(1), || repo.ticks().len() == 1).await;
    let ticks = repo.ticks();
    assert_eq!(ticks[0].market_id, market_id);
    assert_eq!(ticks[0].source, "coinbase");
    assert_eq!(bridge.snapshot().enqueued, 1);
    assert_eq!(bridge.snapshot().missing_market, 0);

    drop(tx);
    drop(writer);
    writer_task.abort();
}

#[tokio::test]
async fn storage_bridge_reports_missing_market_without_panicking() {
    let repo = Arc::new(CapturedRepository::default());
    let (writer, writer_task) = StorageWriter::spawn(repo.clone(), 8, StdDuration::from_millis(5));
    let (tx, rx) = mpsc::channel(8);
    let bridge = RealtimeStorageBridge::spawn(rx, writer.clone(), BTreeMap::new());

    tx.try_send(RealtimeEvent::Tick(tick_at(10, "68000.25")))
        .expect("bridge input should accept tick");

    wait_until(StdDuration::from_secs(1), || {
        bridge.snapshot().missing_market == 1
    })
    .await;
    assert!(repo.ticks().is_empty());
    assert_eq!(bridge.snapshot().enqueued, 0);

    drop(tx);
    drop(writer);
    writer_task.abort();
}

#[derive(Default)]
struct CapturedRepository {
    ticks: Mutex<Vec<NewTick>>,
}

impl CapturedRepository {
    fn ticks(&self) -> Vec<NewTick> {
        self.ticks.lock().expect("ticks lock poisoned").clone()
    }
}

#[async_trait::async_trait]
impl StorageRepository for CapturedRepository {
    async fn insert_raw_market_event(
        &self,
        _event: &NewRawMarketEvent,
    ) -> Result<RawMarketEventRecord, StorageError> {
        unreachable!("bridge test does not insert raw events")
    }

    async fn insert_tick(&self, tick: &NewTick) -> Result<TickRecord, StorageError> {
        self.ticks
            .lock()
            .expect("ticks lock poisoned")
            .push(tick.clone());
        Ok(TickRecord {
            id: Uuid::new_v4(),
            market_id: tick.market_id,
            source: tick.source.clone(),
            source_ts: tick.source_ts,
            received_at: tick.received_at,
            price: tick.price.clone(),
            size: tick.size.clone(),
            sequence: tick.sequence,
        })
    }

    async fn insert_signal(&self, _signal: &NewSignal) -> Result<SignalRecord, StorageError> {
        unreachable!("bridge test does not insert signals")
    }

    async fn insert_notification_delivery(
        &self,
        _delivery: &NewNotificationDelivery,
    ) -> Result<Uuid, StorageError> {
        unreachable!("bridge test does not insert notifications")
    }

    async fn insert_runtime_event(
        &self,
        _event: &NewRuntimeEvent,
    ) -> Result<RuntimeEventRecord, StorageError> {
        unreachable!("bridge test does not insert runtime events")
    }

    async fn replay_ticks_for_window(
        &self,
        _market_key: &str,
        _window_start: OffsetDateTime,
    ) -> Result<Vec<ReplayTick>, StorageError> {
        unreachable!("bridge test does not replay ticks")
    }

    async fn recent_candles(
        &self,
        _market_key: &str,
        _limit: i64,
    ) -> Result<Vec<CandleRecord>, StorageError> {
        unreachable!("bridge test does not query candles")
    }

    async fn latest_signals(
        &self,
        _market_key: &str,
        _limit: i64,
    ) -> Result<Vec<SignalWithMarketRecord>, StorageError> {
        unreachable!("bridge test does not query signals")
    }

    async fn list_model_assignments(&self) -> Result<Vec<ModelAssignmentRecord>, StorageError> {
        unreachable!("bridge test does not query model assignments")
    }

    async fn set_active_model_assignment(
        &self,
        _assignment: &NewModelAssignment,
    ) -> Result<ModelAssignmentRecord, StorageError> {
        unreachable!("bridge test does not set model assignments")
    }

    async fn list_notification_channels(
        &self,
        _market_key: &str,
    ) -> Result<Vec<NotificationChannelRecord>, StorageError> {
        unreachable!("bridge test does not query notification channels")
    }

    async fn upsert_notification_channel(
        &self,
        _channel: &NewNotificationChannel,
    ) -> Result<NotificationChannelRecord, StorageError> {
        unreachable!("bridge test does not upsert notification channels")
    }
}

async fn wait_until<F>(timeout: StdDuration, condition: F)
where
    F: Fn() -> bool,
{
    let started_at = tokio::time::Instant::now();
    while started_at.elapsed() < timeout {
        if condition() {
            return;
        }
        tokio::time::sleep(StdDuration::from_millis(5)).await;
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
        price: BigDecimal::from_str(price).unwrap(),
        size: Some(BigDecimal::from_str("0.1").unwrap()),
        sequence: Some(seconds),
    }
}
