use std::{
    sync::{
        atomic::{AtomicI64, AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

use async_trait::async_trait;
use polymarket_backend::storage::{
    CandleRecord, NewNotificationDelivery, NewRawMarketEvent, NewRuntimeEvent, NewSignal, NewTick,
    RawMarketEventRecord, ReplayTick, RuntimeEventRecord, SignalRecord, SignalWithMarketRecord,
    StorageCommand, StorageError, StorageRepository, StorageWriter, TickRecord,
};
use serde_json::json;
use time::OffsetDateTime;
use uuid::Uuid;

#[tokio::test]
async fn storage_writer_try_enqueue_does_not_block_when_capacity_available() {
    let repo = Arc::new(FakeRepository::default());
    let (handle, join) = StorageWriter::spawn(repo.clone(), 8, Duration::from_millis(10));

    handle
        .try_enqueue(StorageCommand::RuntimeEvent(runtime_event()))
        .expect("enqueue should succeed");

    wait_until(Duration::from_secs(1), || handle.snapshot().written == 1).await;
    let snapshot = handle.snapshot();

    assert_eq!(snapshot.accepted, 1);
    assert_eq!(snapshot.dropped, 0);
    assert_eq!(snapshot.written, 1);
    assert_eq!(repo.runtime_events.load(Ordering::Relaxed), 1);

    drop(handle);
    join.abort();
}

#[tokio::test]
async fn storage_writer_reports_queue_full_without_blocking() {
    let repo = Arc::new(FakeRepository::with_delay(Duration::from_millis(200)));
    let (handle, join) = StorageWriter::spawn(repo, 1, Duration::from_secs(1));

    let mut full_seen = false;
    for _ in 0..100 {
        if matches!(
            handle.try_enqueue(StorageCommand::RuntimeEvent(runtime_event())),
            Err(StorageError::QueueFull)
        ) {
            full_seen = true;
            break;
        }
    }

    let snapshot = handle.snapshot();
    assert!(full_seen, "bounded channel should eventually report full");
    assert!(snapshot.dropped >= 1);

    drop(handle);
    join.abort();
}

#[tokio::test]
async fn storage_writer_records_failed_database_writes() {
    let repo = Arc::new(FakeRepository::failing());
    let (handle, join) = StorageWriter::spawn(repo, 4, Duration::from_millis(10));

    handle
        .try_enqueue(StorageCommand::RuntimeEvent(runtime_event()))
        .expect("enqueue should succeed");

    wait_until(Duration::from_secs(1), || handle.snapshot().failed == 1).await;
    let snapshot = handle.snapshot();

    assert_eq!(snapshot.accepted, 1);
    assert_eq!(snapshot.written, 0);
    assert_eq!(snapshot.failed, 1);
    assert_eq!(snapshot.retried, 2);

    drop(handle);
    join.await
        .expect("writer should stop after handle is dropped");
}

#[tokio::test]
async fn storage_writer_retries_transient_failures_before_counting_written() {
    let repo = Arc::new(FakeRepository::failing_first(2));
    let (handle, join) = StorageWriter::spawn(repo, 4, Duration::from_millis(1));

    handle
        .try_enqueue(StorageCommand::RuntimeEvent(runtime_event()))
        .expect("enqueue should succeed");

    wait_until(Duration::from_millis(250), || {
        handle.snapshot().written == 1
    })
    .await;
    let snapshot = handle.snapshot();

    assert_eq!(snapshot.accepted, 1);
    assert_eq!(snapshot.written, 1);
    assert_eq!(snapshot.failed, 0);
    assert_eq!(snapshot.retried, 2);

    drop(handle);
    join.await
        .expect("writer should stop after handle is dropped");
}

#[tokio::test]
async fn storage_writer_exits_when_last_handle_is_dropped() {
    let repo = Arc::new(FakeRepository::default());
    let (handle, join) = StorageWriter::spawn(repo, 4, Duration::from_millis(10));

    drop(handle);

    tokio::time::timeout(Duration::from_millis(100), join)
        .await
        .expect("writer should exit promptly")
        .expect("writer task should finish cleanly");
}

async fn wait_until<F>(timeout: Duration, condition: F)
where
    F: Fn() -> bool,
{
    let started_at = tokio::time::Instant::now();
    while started_at.elapsed() < timeout {
        if condition() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

fn runtime_event() -> NewRuntimeEvent {
    NewRuntimeEvent {
        component: "test".to_string(),
        severity: "info".to_string(),
        event_type: "writer_test".to_string(),
        message: "test event".to_string(),
        details: json!({"test": true}),
    }
}

#[derive(Default)]
struct FakeRepository {
    runtime_events: AtomicU64,
    failures_remaining: AtomicI64,
    delay: Option<Duration>,
}

impl FakeRepository {
    fn failing() -> Self {
        Self::failing_first(3)
    }

    fn failing_first(count: i64) -> Self {
        Self {
            runtime_events: AtomicU64::new(0),
            failures_remaining: AtomicI64::new(count),
            delay: None,
        }
    }

    fn with_delay(delay: Duration) -> Self {
        Self {
            runtime_events: AtomicU64::new(0),
            failures_remaining: AtomicI64::new(0),
            delay: Some(delay),
        }
    }

    async fn maybe_fail(&self) -> Result<(), StorageError> {
        if let Some(delay) = self.delay {
            tokio::time::sleep(delay).await;
        }

        let previous_failures = self.failures_remaining.fetch_sub(1, Ordering::Relaxed);
        if previous_failures > 0 {
            Err(StorageError::InvalidInput("forced failure".to_string()))
        } else {
            Ok(())
        }
    }
}

#[async_trait]
impl StorageRepository for FakeRepository {
    async fn insert_raw_market_event(
        &self,
        event: &NewRawMarketEvent,
    ) -> Result<RawMarketEventRecord, StorageError> {
        self.maybe_fail().await?;
        Ok(RawMarketEventRecord {
            id: Uuid::new_v4(),
            source: event.source.clone(),
            source_event_id: event.source_event_id.clone(),
            received_at: event.received_at,
            source_ts: event.source_ts,
            payload: event.payload.clone(),
        })
    }

    async fn insert_tick(&self, tick: &NewTick) -> Result<TickRecord, StorageError> {
        self.maybe_fail().await?;
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

    async fn insert_signal(&self, signal: &NewSignal) -> Result<SignalRecord, StorageError> {
        self.maybe_fail().await?;
        Ok(SignalRecord {
            id: Uuid::new_v4(),
            market_window_id: signal.market_window_id,
            model_version_id: signal.model_version_id,
            signal_type: signal.signal_type.clone(),
            side: signal.side.clone(),
            confidence: signal.confidence.clone(),
            limit_price: signal.limit_price.clone(),
            suggested_size: signal.suggested_size.clone(),
            ttl_ms: signal.ttl_ms,
            reason: signal.reason.clone(),
            features: signal.features.clone(),
            input_snapshot_hash: signal.input_snapshot_hash.clone(),
            created_at: OffsetDateTime::UNIX_EPOCH,
        })
    }

    async fn insert_notification_delivery(
        &self,
        _delivery: &NewNotificationDelivery,
    ) -> Result<Uuid, StorageError> {
        self.maybe_fail().await?;
        Ok(Uuid::new_v4())
    }

    async fn insert_runtime_event(
        &self,
        event: &NewRuntimeEvent,
    ) -> Result<RuntimeEventRecord, StorageError> {
        self.maybe_fail().await?;
        self.runtime_events.fetch_add(1, Ordering::Relaxed);
        Ok(RuntimeEventRecord {
            id: Uuid::new_v4(),
            component: event.component.clone(),
            severity: event.severity.clone(),
            event_type: event.event_type.clone(),
            message: event.message.clone(),
            details: event.details.clone(),
            created_at: OffsetDateTime::UNIX_EPOCH,
        })
    }

    async fn replay_ticks_for_window(
        &self,
        _market_key: &str,
        _window_start: OffsetDateTime,
    ) -> Result<Vec<ReplayTick>, StorageError> {
        self.maybe_fail().await?;
        Ok(Vec::new())
    }

    async fn recent_candles(
        &self,
        _market_key: &str,
        _limit: i64,
    ) -> Result<Vec<CandleRecord>, StorageError> {
        self.maybe_fail().await?;
        Ok(Vec::new())
    }

    async fn latest_signals(
        &self,
        _market_key: &str,
        _limit: i64,
    ) -> Result<Vec<SignalWithMarketRecord>, StorageError> {
        self.maybe_fail().await?;
        Ok(Vec::new())
    }
}
