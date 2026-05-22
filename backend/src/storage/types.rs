use bigdecimal::BigDecimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::FromRow;
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, FromRow)]
pub struct AssetRecord {
    pub id: Uuid,
    pub symbol: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, FromRow)]
pub struct MarketRecord {
    pub id: Uuid,
    pub market_key: String,
    pub asset_id: Uuid,
    pub symbol: String,
    pub interval_seconds: i32,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NewRawMarketEvent {
    pub source: String,
    pub source_event_id: Option<String>,
    pub received_at: OffsetDateTime,
    pub source_ts: Option<OffsetDateTime>,
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, FromRow)]
pub struct RawMarketEventRecord {
    pub id: Uuid,
    pub source: String,
    pub source_event_id: Option<String>,
    pub received_at: OffsetDateTime,
    pub source_ts: Option<OffsetDateTime>,
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NewTick {
    pub market_id: Uuid,
    pub source: String,
    pub source_ts: OffsetDateTime,
    pub received_at: OffsetDateTime,
    pub price: BigDecimal,
    pub size: Option<BigDecimal>,
    pub sequence: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, FromRow)]
pub struct TickRecord {
    pub id: Uuid,
    pub market_id: Uuid,
    pub source: String,
    pub source_ts: OffsetDateTime,
    pub received_at: OffsetDateTime,
    pub price: BigDecimal,
    pub size: Option<BigDecimal>,
    pub sequence: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NewSignal {
    pub market_window_id: Uuid,
    pub model_version_id: Uuid,
    pub signal_type: String,
    pub side: Option<String>,
    pub confidence: Option<BigDecimal>,
    pub limit_price: Option<BigDecimal>,
    pub suggested_size: Option<BigDecimal>,
    pub ttl_ms: Option<i32>,
    pub reason: String,
    pub features: Value,
    pub input_snapshot_hash: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, FromRow)]
pub struct SignalRecord {
    pub id: Uuid,
    pub market_window_id: Uuid,
    pub model_version_id: Uuid,
    pub signal_type: String,
    pub side: Option<String>,
    pub confidence: Option<BigDecimal>,
    pub limit_price: Option<BigDecimal>,
    pub suggested_size: Option<BigDecimal>,
    pub ttl_ms: Option<i32>,
    pub reason: String,
    pub features: Value,
    pub input_snapshot_hash: String,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, FromRow)]
pub struct CandleRecord {
    pub market_key: String,
    pub start_ts: OffsetDateTime,
    pub open: BigDecimal,
    pub high: BigDecimal,
    pub low: BigDecimal,
    pub close: BigDecimal,
    pub volume: BigDecimal,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, FromRow)]
pub struct SignalWithMarketRecord {
    pub id: Uuid,
    pub market_key: String,
    pub market_window_id: Uuid,
    pub model_version_id: Uuid,
    pub signal_type: String,
    pub side: Option<String>,
    pub confidence: Option<BigDecimal>,
    pub limit_price: Option<BigDecimal>,
    pub suggested_size: Option<BigDecimal>,
    pub ttl_ms: Option<i32>,
    pub reason: String,
    pub features: Value,
    pub input_snapshot_hash: String,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, FromRow)]
pub struct SignalNotificationMetadata {
    pub window_start: OffsetDateTime,
    pub window_end: OffsetDateTime,
    pub model_key: String,
    pub model_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, FromRow)]
pub struct ModelAssignmentRecord {
    pub market_key: String,
    pub model_key: String,
    pub display_name: String,
    pub version: String,
    pub parameters: Value,
    pub status: String,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewModelAssignment {
    pub market_key: String,
    pub model_key: String,
    pub display_name: String,
    pub version: String,
    pub parameters: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, FromRow)]
pub struct NotificationChannelRecord {
    pub id: Uuid,
    pub market_key: String,
    pub channel_type: String,
    pub name: String,
    pub webhook_url: String,
    pub enabled: bool,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewNotificationChannel {
    pub market_key: String,
    pub channel_type: String,
    pub name: String,
    pub webhook_url: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewRuntimeEvent {
    pub component: String,
    pub severity: String,
    pub event_type: String,
    pub message: String,
    pub details: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewBacktestRun {
    pub market_key: String,
    pub model_key: String,
    pub display_name: String,
    pub model_version: String,
    pub parameters: Value,
    pub window_start: OffsetDateTime,
    pub window_end: OffsetDateTime,
    pub metrics: Value,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, FromRow)]
pub struct BacktestRunRecord {
    pub id: Uuid,
    pub market_key: String,
    pub model_key: String,
    pub display_name: String,
    pub model_version: String,
    pub parameters: Value,
    pub started_at: OffsetDateTime,
    pub finished_at: Option<OffsetDateTime>,
    pub window_start: OffsetDateTime,
    pub window_end: OffsetDateTime,
    pub metrics: Value,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, FromRow)]
pub struct RuntimeEventRecord {
    pub id: Uuid,
    pub component: String,
    pub severity: String,
    pub event_type: String,
    pub message: String,
    pub details: Value,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NewNotificationDelivery {
    pub signal_id: Uuid,
    pub channel_id: Uuid,
    pub dedupe_key: String,
    pub status: String,
    pub attempt_count: i32,
    pub response_summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, FromRow)]
pub struct NotificationDeliveryRecord {
    pub id: Uuid,
    pub market_key: String,
    pub signal_id: Uuid,
    pub channel_id: Uuid,
    pub channel_type: String,
    pub channel_name: String,
    pub status: String,
    pub attempt_count: i32,
    pub response_summary: Option<String>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReplayTick {
    pub market_key: String,
    pub window_start: OffsetDateTime,
    pub window_end: OffsetDateTime,
    pub source_ts: OffsetDateTime,
    pub price: BigDecimal,
    pub size: Option<BigDecimal>,
}
