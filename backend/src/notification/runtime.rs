use std::{
    collections::HashSet,
    sync::{Arc, Mutex, RwLock},
    time::Duration,
};

use async_trait::async_trait;
use serde_json::Value;
use tokio::{sync::mpsc, task::JoinHandle, time};

use crate::{
    notification::types::DeliveryAudit,
    notification::{
        render_feishu_card, FeishuCardInput, NotificationChannelView, NotificationError,
        NotificationJob, NotificationRuntimeConfig, NotificationRuntimeSnapshot,
        NotificationSendOutcome,
    },
    storage::{
        NewNotificationDelivery, NotificationChannelRecord, StorageCommand, StorageWriterHandle,
    },
};

#[async_trait]
pub trait NotificationSender: Send + Sync {
    async fn send(
        &self,
        webhook_url: &str,
        payload: Value,
    ) -> Result<NotificationSendOutcome, NotificationError>;
}

#[derive(Clone)]
pub struct NotificationRuntime {
    tx: mpsc::Sender<NotificationJob>,
    metrics: Arc<RwLock<NotificationRuntimeSnapshot>>,
    _task: Option<Arc<JoinHandle<()>>>,
    _rx_guard: Option<Arc<Mutex<mpsc::Receiver<NotificationJob>>>>,
}

impl NotificationRuntime {
    pub fn spawn<S>(
        sender: Arc<S>,
        storage_writer: StorageWriterHandle,
        capacity: usize,
        config: NotificationRuntimeConfig,
    ) -> Self
    where
        S: NotificationSender + 'static,
    {
        let sender: Arc<dyn NotificationSender> = sender;
        let (tx, mut rx) = mpsc::channel::<NotificationJob>(capacity);
        let metrics = Arc::new(RwLock::new(NotificationRuntimeSnapshot::default()));
        let dedupe = Arc::new(Mutex::new(HashSet::<String>::new()));
        let worker_metrics = metrics.clone();
        let worker_dedupe = dedupe.clone();
        let task = tokio::spawn(async move {
            while let Some(job) = rx.recv().await {
                process_job(
                    job,
                    sender.as_ref(),
                    &storage_writer,
                    &config,
                    &worker_metrics,
                    &worker_dedupe,
                )
                .await;
            }
        });

        Self {
            tx,
            metrics,
            _task: Some(Arc::new(task)),
            _rx_guard: None,
        }
    }

    pub fn spawn_paused_for_tests<S>(
        _sender: Arc<S>,
        _storage_writer: StorageWriterHandle,
        capacity: usize,
        _config: NotificationRuntimeConfig,
    ) -> Self
    where
        S: NotificationSender + 'static,
    {
        let (tx, rx) = mpsc::channel::<NotificationJob>(capacity);
        Self {
            tx,
            metrics: Arc::new(RwLock::new(NotificationRuntimeSnapshot::default())),
            _task: None,
            _rx_guard: Some(Arc::new(Mutex::new(rx))),
        }
    }

    pub fn try_enqueue(&self, job: NotificationJob) -> Result<(), NotificationError> {
        match self.tx.try_send(job) {
            Ok(()) => {
                self.update_metrics(|metrics| metrics.accepted += 1);
                Ok(())
            }
            Err(mpsc::error::TrySendError::Full(_)) => {
                self.update_metrics(|metrics| metrics.dropped += 1);
                Err(NotificationError::QueueFull)
            }
            Err(mpsc::error::TrySendError::Closed(_)) => Err(NotificationError::QueueClosed),
        }
    }

    pub fn snapshot(&self) -> NotificationRuntimeSnapshot {
        self.metrics
            .read()
            .expect("notification metrics lock poisoned")
            .clone()
    }

    fn update_metrics(&self, update: impl FnOnce(&mut NotificationRuntimeSnapshot)) {
        let mut metrics = self
            .metrics
            .write()
            .expect("notification metrics lock poisoned");
        update(&mut metrics);
    }
}

async fn process_job(
    job: NotificationJob,
    sender: &dyn NotificationSender,
    storage_writer: &StorageWriterHandle,
    config: &NotificationRuntimeConfig,
    metrics: &Arc<RwLock<NotificationRuntimeSnapshot>>,
    dedupe: &Arc<Mutex<HashSet<String>>>,
) {
    update_metrics(metrics, |metrics| metrics.processed += 1);
    for channel in job.channels.iter().filter(|channel| {
        channel.enabled
            && channel.channel_type == "feishu"
            && channel.market_key == job.signal.market_key
    }) {
        let dedupe_key = dedupe_key(&job, channel);
        if !mark_dedupe(dedupe, &dedupe_key) {
            update_metrics(metrics, |metrics| metrics.deduped += 1);
            continue;
        }

        let input = card_input(&job, channel);
        let payload = render_feishu_card(&input);
        let (audit, sent) =
            send_with_retry(sender, channel, payload, &dedupe_key, &job, config, metrics).await;
        if sent {
            update_metrics(metrics, |metrics| metrics.sent += 1);
        } else {
            update_metrics(metrics, |metrics| metrics.failed += 1);
        }
        if storage_writer
            .try_enqueue(StorageCommand::NotificationDelivery(
                NewNotificationDelivery {
                    signal_id: audit.signal_id,
                    channel_id: audit.channel_id,
                    dedupe_key: audit.dedupe_key,
                    status: audit.status,
                    attempt_count: audit.attempt_count,
                    response_summary: audit.response_summary,
                },
            ))
            .is_err()
        {
            update_metrics(metrics, |metrics| metrics.audit_dropped += 1);
        }
    }
}

async fn send_with_retry(
    sender: &dyn NotificationSender,
    channel: &NotificationChannelRecord,
    payload: Value,
    dedupe_key: &str,
    job: &NotificationJob,
    config: &NotificationRuntimeConfig,
    metrics: &Arc<RwLock<NotificationRuntimeSnapshot>>,
) -> (DeliveryAudit, bool) {
    let max_attempts = config.max_attempts.max(1);
    let mut attempt = 1_u8;
    loop {
        let result = time::timeout(
            Duration::from_millis(config.send_timeout_ms.max(1)),
            sender.send(&channel.webhook_url, payload.clone()),
        )
        .await
        .map_err(|_| NotificationError::Timeout)
        .and_then(|result| result);

        match result {
            Ok(outcome) => {
                return (
                    DeliveryAudit {
                        signal_id: job.signal.id,
                        channel_id: channel.id,
                        dedupe_key: dedupe_key.to_string(),
                        status: "sent".to_string(),
                        attempt_count: i32::from(attempt),
                        response_summary: outcome.response_summary,
                    },
                    true,
                );
            }
            Err(error) => {
                let error_summary = error.to_string();
                if attempt >= max_attempts {
                    return (
                        DeliveryAudit {
                            signal_id: job.signal.id,
                            channel_id: channel.id,
                            dedupe_key: dedupe_key.to_string(),
                            status: "failed".to_string(),
                            attempt_count: i32::from(attempt),
                            response_summary: Some(error_summary),
                        },
                        false,
                    );
                }
                update_metrics(metrics, |metrics| metrics.retried += 1);
                attempt += 1;
                time::sleep(Duration::from_millis(
                    config.retry_base_delay_ms.max(1) * u64::from(attempt),
                ))
                .await;
            }
        }
    }
}

fn mark_dedupe(dedupe: &Arc<Mutex<HashSet<String>>>, dedupe_key: &str) -> bool {
    dedupe
        .lock()
        .expect("notification dedupe lock poisoned")
        .insert(dedupe_key.to_string())
}

fn update_metrics(
    metrics: &Arc<RwLock<NotificationRuntimeSnapshot>>,
    update: impl FnOnce(&mut NotificationRuntimeSnapshot),
) {
    let mut metrics = metrics.write().expect("notification metrics lock poisoned");
    update(&mut metrics);
}

fn dedupe_key(job: &NotificationJob, channel: &NotificationChannelRecord) -> String {
    format!("{}:{}", job.signal.id, channel.id)
}

fn card_input(job: &NotificationJob, channel: &NotificationChannelRecord) -> FeishuCardInput {
    FeishuCardInput {
        market_key: job.signal.market_key.clone(),
        market_label: market_label(&job.signal.market_key),
        window_start: job.signal.created_at,
        window_end: job.signal.created_at,
        side: job
            .signal
            .side
            .clone()
            .unwrap_or_else(|| "UNKNOWN".to_string())
            .to_ascii_uppercase(),
        confidence: job
            .signal
            .confidence
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_else(|| "n/a".to_string()),
        limit_price: job
            .signal
            .limit_price
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_else(|| "n/a".to_string()),
        suggested_size: job
            .signal
            .suggested_size
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_else(|| "n/a".to_string()),
        ttl_ms: job.signal.ttl_ms.unwrap_or(0),
        model_key: "model".to_string(),
        model_version: job.signal.model_version_id.to_string(),
        reason: job.signal.reason.clone(),
        features: job.signal.features.clone(),
        channel: NotificationChannelView {
            name: channel.name.clone(),
            webhook_url_masked: mask_webhook_url(&channel.webhook_url),
            webhook_url: channel.webhook_url.clone(),
        },
    }
}

fn market_label(market_key: &str) -> String {
    match market_key {
        "btc5m" => "BTC 5m".to_string(),
        "eth15m" => "ETH 15m".to_string(),
        other => other.to_string(),
    }
}

fn mask_webhook_url(webhook_url: &str) -> String {
    let suffix = webhook_url
        .chars()
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();
    if let Some((scheme, rest)) = webhook_url.split_once("://") {
        let host = rest.split('/').next().unwrap_or("webhook");
        format!("{scheme}://{host}/.../{suffix}")
    } else {
        format!(".../{suffix}")
    }
}
