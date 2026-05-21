use std::{
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

use async_trait::async_trait;
use polymarket_backend::storage::{
    NewNotificationDelivery, NewRuntimeEvent, NewSignal, NewTick, ReplayTick, RuntimeEventRecord,
    SignalRecord, StorageCommand, StorageError, StorageRepository, StorageWriter, TickRecord,
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

    tokio::time::sleep(Duration::from_millis(40)).await;
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

    tokio::time::sleep(Duration::from_millis(40)).await;
    let snapshot = handle.snapshot();

    assert_eq!(snapshot.accepted, 1);
    assert_eq!(snapshot.written, 0);
    assert_eq!(snapshot.failed, 1);

    drop(handle);
    join.abort();
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
    fail: AtomicBool,
    delay: Option<Duration>,
}

impl FakeRepository {
    fn failing() -> Self {
        Self {
            runtime_events: AtomicU64::new(0),
            fail: AtomicBool::new(true),
            delay: None,
        }
    }

    fn with_delay(delay: Duration) -> Self {
        Self {
            runtime_events: AtomicU64::new(0),
            fail: AtomicBool::new(false),
            delay: Some(delay),
        }
    }

    async fn maybe_fail(&self) -> Result<(), StorageError> {
        if let Some(delay) = self.delay {
            tokio::time::sleep(delay).await;
        }

        if self.fail.load(Ordering::Relaxed) {
            Err(StorageError::InvalidInput("forced failure".to_string()))
        } else {
            Ok(())
        }
    }
}

#[async_trait]
impl StorageRepository for FakeRepository {
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
}
