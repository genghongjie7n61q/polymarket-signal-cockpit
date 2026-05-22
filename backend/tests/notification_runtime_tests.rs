use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

use async_trait::async_trait;
use polymarket_backend::{
    notification::{
        NotificationJob, NotificationRuntime, NotificationRuntimeConfig, NotificationSendOutcome,
        NotificationSender, SignalNotificationBridge,
    },
    storage::{
        BacktestRunRecord, CandleRecord, ModelAssignmentRecord, NewBacktestRun, NewModelAssignment,
        NewNotificationChannel, NewNotificationDelivery, NewRawMarketEvent, NewRuntimeEvent,
        NewSignal, NewTick, NotificationChannelRecord, RawMarketEventRecord, ReplayTick,
        RuntimeEventRecord, SignalRecord, SignalWithMarketRecord, StorageError, StorageRepository,
        StorageWriter, TickRecord,
    },
};
use serde_json::Value;
use time::OffsetDateTime;
use uuid::Uuid;

#[tokio::test]
async fn notification_runtime_reports_queue_full_without_blocking() {
    let (storage_writer, writer_task) = storage_writer();
    let sender = Arc::new(FakeSender::default());
    let runtime =
        NotificationRuntime::spawn_paused_for_tests(sender, storage_writer, 1, test_config());

    runtime.try_enqueue(job()).expect("first enqueue");
    let error = runtime.try_enqueue(job()).expect_err("second enqueue");

    assert_eq!(error.to_string(), "notification queue is full");
    let snapshot = runtime.snapshot();
    assert_eq!(snapshot.accepted, 1);
    assert_eq!(snapshot.dropped, 1);

    writer_task.abort();
}

#[tokio::test]
async fn notification_runtime_sends_enabled_feishu_channels_and_audits_deliveries() {
    let (storage_writer, writer_task, repository) = storage_writer_with_repository();
    let sender = Arc::new(FakeSender::default());
    let runtime = NotificationRuntime::spawn(sender.clone(), storage_writer, 8, test_config());
    let job = job();
    let market_window_id = job.signal.market_window_id;

    runtime.try_enqueue(job).expect("enqueue");

    wait_until(Duration::from_secs(1), || sender.requests().len() == 2).await;
    wait_until(Duration::from_secs(1), || {
        repository.deliveries().len() == 2
    })
    .await;

    let requests = sender.requests();
    assert_eq!(requests.len(), 2);
    assert!(requests
        .iter()
        .all(|request| request.webhook_url.starts_with("https://open.feishu.cn/")));
    assert!(requests
        .iter()
        .all(|request| request.payload["msg_type"] == "interactive"));
    assert!(requests
        .iter()
        .all(|request| !request.payload.to_string().contains("secret-token")));

    let deliveries = repository.deliveries();
    assert_eq!(deliveries.len(), 2);
    assert!(deliveries.iter().all(|delivery| delivery.status == "sent"));
    assert!(deliveries
        .iter()
        .all(|delivery| delivery.attempt_count == 1));
    assert!(deliveries
        .iter()
        .all(|delivery| delivery.dedupe_key.contains(&market_window_id.to_string())));

    drop(runtime);
    writer_task.abort();
}

#[tokio::test]
async fn notification_runtime_retries_transient_sender_failures() {
    let (storage_writer, writer_task, repository) = storage_writer_with_repository();
    let sender = Arc::new(FakeSender::with_failures(2));
    let runtime = NotificationRuntime::spawn(
        sender.clone(),
        storage_writer,
        8,
        NotificationRuntimeConfig {
            max_attempts: 3,
            send_timeout_ms: 200,
            retry_base_delay_ms: 1,
        },
    );

    runtime
        .try_enqueue(single_channel_job())
        .expect("enqueue retry job");

    wait_until(Duration::from_secs(1), || sender.requests().len() == 3).await;
    wait_until(Duration::from_secs(1), || {
        repository.deliveries().len() == 1
    })
    .await;

    let snapshot = runtime.snapshot();
    assert_eq!(snapshot.sent, 1);
    assert_eq!(snapshot.retried, 2);
    assert_eq!(snapshot.failed, 0);
    assert_eq!(repository.deliveries()[0].attempt_count, 3);
    assert_eq!(repository.deliveries()[0].status, "sent");

    drop(runtime);
    writer_task.abort();
}

#[tokio::test]
async fn notification_runtime_dedupes_same_signal_channel() {
    let (storage_writer, writer_task, repository) = storage_writer_with_repository();
    let sender = Arc::new(FakeSender::default());
    let runtime = NotificationRuntime::spawn(sender.clone(), storage_writer, 8, test_config());
    let job = single_channel_job();

    runtime.try_enqueue(job.clone()).expect("first enqueue");
    runtime.try_enqueue(job).expect("duplicate enqueue");

    wait_until(Duration::from_secs(1), || runtime.snapshot().deduped == 1).await;
    wait_until(Duration::from_secs(1), || {
        repository.deliveries().len() == 1
    })
    .await;

    assert_eq!(sender.requests().len(), 1);
    assert_eq!(runtime.snapshot().sent, 1);

    drop(runtime);
    writer_task.abort();
}

#[tokio::test]
async fn notification_runtime_dedupes_semantically_equivalent_regenerated_signals() {
    let (storage_writer, writer_task, repository) = storage_writer_with_repository();
    let sender = Arc::new(FakeSender::default());
    let runtime = NotificationRuntime::spawn(sender.clone(), storage_writer, 8, test_config());
    let first = single_channel_job();
    let mut second = first.clone();
    second.signal.id = Uuid::new_v4();

    runtime.try_enqueue(first).expect("first enqueue");
    runtime.try_enqueue(second).expect("regenerated enqueue");

    wait_until(Duration::from_secs(1), || runtime.snapshot().deduped == 1).await;
    wait_until(Duration::from_secs(1), || {
        repository.deliveries().len() == 1
    })
    .await;

    assert_eq!(sender.requests().len(), 1);

    drop(runtime);
    writer_task.abort();
}

#[tokio::test]
async fn signal_notification_bridge_enqueues_actionable_signals_for_enabled_channels() {
    let repository = Arc::new(CapturedRepository::with_channels(vec![channel(
        "btc5m",
        "primary",
        true,
        "https://open.feishu.cn/open-apis/bot/v2/hook/secret-token",
    )]));
    let (storage_writer, writer_task) =
        StorageWriter::spawn(repository.clone(), 8, Duration::from_millis(5));
    let sender = Arc::new(FakeSender::default());
    let runtime = NotificationRuntime::spawn(sender.clone(), storage_writer, 8, test_config());
    let (tx, rx) = tokio::sync::mpsc::channel(8);
    let _bridge = SignalNotificationBridge::spawn(rx, repository.clone(), runtime.clone());

    tx.send(signal(Uuid::new_v4(), "btc5m"))
        .await
        .expect("signal handoff");

    wait_until(Duration::from_secs(1), || sender.requests().len() == 1).await;
    wait_until(Duration::from_secs(1), || {
        repository.deliveries().len() == 1
    })
    .await;

    assert_eq!(runtime.snapshot().accepted, 1);

    drop(runtime);
    writer_task.abort();
}

#[test]
fn notification_error_summary_redacts_feishu_webhook_secret() {
    let error = polymarket_backend::notification::NotificationError::SendFailed(
        "request error for https://open.feishu.cn/open-apis/bot/v2/hook/secret-token".to_string(),
    );

    let summary = error.safe_summary();

    assert!(summary.contains("/hook/****"));
    assert!(!summary.contains("secret-token"));
}

fn test_config() -> NotificationRuntimeConfig {
    NotificationRuntimeConfig {
        max_attempts: 2,
        send_timeout_ms: 200,
        retry_base_delay_ms: 1,
    }
}

fn job() -> NotificationJob {
    let signal_id = Uuid::new_v4();
    NotificationJob {
        signal: signal(signal_id, "btc5m"),
        channels: vec![
            channel(
                "btc5m",
                "primary",
                true,
                "https://open.feishu.cn/open-apis/bot/v2/hook/secret-token",
            ),
            channel(
                "btc5m",
                "backup",
                true,
                "https://open.feishu.cn/open-apis/bot/v2/hook/backup-secret",
            ),
            channel(
                "btc5m",
                "disabled",
                false,
                "https://open.feishu.cn/open-apis/bot/v2/hook/disabled",
            ),
            NotificationChannelRecord {
                channel_type: "email".to_string(),
                ..channel("btc5m", "email", true, "https://example.invalid")
            },
        ],
    }
}

fn single_channel_job() -> NotificationJob {
    let signal_id = Uuid::new_v4();
    NotificationJob {
        signal: signal(signal_id, "eth15m"),
        channels: vec![channel(
            "eth15m",
            "primary",
            true,
            "https://open.feishu.cn/open-apis/bot/v2/hook/secret-token",
        )],
    }
}

fn signal(id: Uuid, market_key: &str) -> SignalWithMarketRecord {
    SignalWithMarketRecord {
        id,
        market_key: market_key.to_string(),
        market_window_id: Uuid::new_v4(),
        model_version_id: Uuid::new_v4(),
        signal_type: "actionable_alert".to_string(),
        side: Some("Up".to_string()),
        confidence: Some("0.68".parse().unwrap()),
        limit_price: Some("0.52".parse().unwrap()),
        suggested_size: Some("1.50".parse().unwrap()),
        ttl_ms: Some(15_000),
        reason: "directional threshold crossed".to_string(),
        features: serde_json::json!({"return_bps": "8.4"}),
        input_snapshot_hash: "snapshot-1".to_string(),
        created_at: OffsetDateTime::UNIX_EPOCH,
    }
}

fn channel(
    market_key: &str,
    name: &str,
    enabled: bool,
    webhook_url: &str,
) -> NotificationChannelRecord {
    NotificationChannelRecord {
        id: Uuid::new_v4(),
        market_key: market_key.to_string(),
        channel_type: "feishu".to_string(),
        name: name.to_string(),
        webhook_url: webhook_url.to_string(),
        enabled,
        created_at: OffsetDateTime::UNIX_EPOCH,
    }
}

#[derive(Debug, Clone)]
struct SentRequest {
    webhook_url: String,
    payload: Value,
}

#[derive(Default)]
struct FakeSender {
    requests: Mutex<Vec<SentRequest>>,
    failures_remaining: AtomicUsize,
}

impl FakeSender {
    fn with_failures(failures: usize) -> Self {
        Self {
            requests: Mutex::new(Vec::new()),
            failures_remaining: AtomicUsize::new(failures),
        }
    }

    fn requests(&self) -> Vec<SentRequest> {
        self.requests.lock().expect("requests lock").clone()
    }
}

#[async_trait]
impl NotificationSender for FakeSender {
    async fn send(
        &self,
        webhook_url: &str,
        payload: Value,
    ) -> Result<NotificationSendOutcome, polymarket_backend::notification::NotificationError> {
        self.requests
            .lock()
            .expect("requests lock")
            .push(SentRequest {
                webhook_url: webhook_url.to_string(),
                payload,
            });
        let previous =
            self.failures_remaining
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                    value.checked_sub(1)
                });
        if matches!(previous, Ok(value) if value > 0) {
            Err(
                polymarket_backend::notification::NotificationError::SendFailed(
                    "temporary failure".to_string(),
                ),
            )
        } else {
            Ok(NotificationSendOutcome {
                response_summary: Some("ok".to_string()),
            })
        }
    }
}

fn storage_writer() -> (
    polymarket_backend::storage::StorageWriterHandle,
    tokio::task::JoinHandle<()>,
) {
    let repository = Arc::new(CapturedRepository::default());
    let (writer, writer_task) = StorageWriter::spawn(repository, 8, Duration::from_millis(5));
    (writer, writer_task)
}

fn storage_writer_with_repository() -> (
    polymarket_backend::storage::StorageWriterHandle,
    tokio::task::JoinHandle<()>,
    Arc<CapturedRepository>,
) {
    let repository = Arc::new(CapturedRepository::default());
    let (writer, writer_task) =
        StorageWriter::spawn(repository.clone(), 8, Duration::from_millis(5));
    (writer, writer_task, repository)
}

#[derive(Default)]
struct CapturedRepository {
    deliveries: Mutex<Vec<NewNotificationDelivery>>,
    channels: Mutex<Vec<NotificationChannelRecord>>,
}

impl CapturedRepository {
    fn with_channels(channels: Vec<NotificationChannelRecord>) -> Self {
        Self {
            deliveries: Mutex::new(Vec::new()),
            channels: Mutex::new(channels),
        }
    }

    fn deliveries(&self) -> Vec<NewNotificationDelivery> {
        self.deliveries.lock().expect("deliveries lock").clone()
    }
}

#[async_trait]
impl StorageRepository for CapturedRepository {
    async fn insert_notification_delivery(
        &self,
        delivery: &NewNotificationDelivery,
    ) -> Result<Uuid, StorageError> {
        self.deliveries
            .lock()
            .expect("deliveries lock")
            .push(delivery.clone());
        Ok(Uuid::new_v4())
    }

    async fn insert_raw_market_event(
        &self,
        _event: &NewRawMarketEvent,
    ) -> Result<RawMarketEventRecord, StorageError> {
        unreachable!("notification test does not insert raw events")
    }

    async fn insert_tick(&self, _tick: &NewTick) -> Result<TickRecord, StorageError> {
        unreachable!("notification test does not insert ticks")
    }

    async fn insert_signal(&self, _signal: &NewSignal) -> Result<SignalRecord, StorageError> {
        unreachable!("notification test does not insert signals")
    }

    async fn insert_runtime_event(
        &self,
        _event: &NewRuntimeEvent,
    ) -> Result<RuntimeEventRecord, StorageError> {
        unreachable!("notification test does not insert runtime events")
    }

    async fn replay_ticks_for_window(
        &self,
        _market_key: &str,
        _window_start: OffsetDateTime,
    ) -> Result<Vec<ReplayTick>, StorageError> {
        unreachable!("notification test does not replay ticks")
    }

    async fn recent_candles(
        &self,
        _market_key: &str,
        _limit: i64,
    ) -> Result<Vec<CandleRecord>, StorageError> {
        unreachable!("notification test does not query candles")
    }

    async fn latest_signals(
        &self,
        _market_key: &str,
        _limit: i64,
    ) -> Result<Vec<SignalWithMarketRecord>, StorageError> {
        unreachable!("notification test does not query signals")
    }

    async fn list_model_assignments(&self) -> Result<Vec<ModelAssignmentRecord>, StorageError> {
        unreachable!("notification test does not query model assignments")
    }

    async fn set_active_model_assignment(
        &self,
        _assignment: &NewModelAssignment,
    ) -> Result<ModelAssignmentRecord, StorageError> {
        unreachable!("notification test does not set model assignments")
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
        _channel: &NewNotificationChannel,
    ) -> Result<NotificationChannelRecord, StorageError> {
        unreachable!("notification test does not upsert notification channels")
    }

    async fn insert_backtest_run(
        &self,
        _run: &NewBacktestRun,
    ) -> Result<BacktestRunRecord, StorageError> {
        unreachable!("notification test does not insert backtest runs")
    }

    async fn latest_backtest_runs(
        &self,
        _market_key: Option<&str>,
        _model_key: Option<&str>,
        _limit: i64,
    ) -> Result<Vec<BacktestRunRecord>, StorageError> {
        unreachable!("notification test does not query backtest runs")
    }
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
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("condition not met before timeout");
}
