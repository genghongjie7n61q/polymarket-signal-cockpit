use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::storage::{NotificationChannelRecord, SignalWithMarketRecord};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotificationChannelView {
    pub name: String,
    pub webhook_url_masked: String,
    pub webhook_url: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeishuCardInput {
    pub market_key: String,
    pub market_label: String,
    pub window_start: OffsetDateTime,
    pub window_end: OffsetDateTime,
    pub side: String,
    pub confidence: String,
    pub limit_price: String,
    pub suggested_size: String,
    pub ttl_ms: i32,
    pub model_key: String,
    pub model_version: String,
    pub reason: String,
    pub features: Value,
    pub channel: NotificationChannelView,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NotificationJob {
    pub signal: SignalWithMarketRecord,
    pub channels: Vec<NotificationChannelRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotificationSendOutcome {
    pub response_summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotificationRuntimeConfig {
    pub max_attempts: u8,
    pub send_timeout_ms: u64,
    pub retry_base_delay_ms: u64,
}

impl Default for NotificationRuntimeConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            send_timeout_ms: 3_000,
            retry_base_delay_ms: 250,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotificationRuntimeSnapshot {
    pub queued_capacity: usize,
    pub queued_available: usize,
    pub task_status: String,
    pub accepted: u64,
    pub dropped: u64,
    pub processed: u64,
    pub sent: u64,
    pub failed: u64,
    pub retried: u64,
    pub deduped: u64,
    pub audit_dropped: u64,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum NotificationError {
    #[error("notification queue is full")]
    QueueFull,
    #[error("notification queue is closed")]
    QueueClosed,
    #[error("notification send failed: {0}")]
    SendFailed(String),
    #[error("notification send timed out")]
    Timeout,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveryAudit {
    pub signal_id: Uuid,
    pub channel_id: Uuid,
    pub dedupe_key: String,
    pub status: String,
    pub attempt_count: i32,
    pub response_summary: Option<String>,
}
