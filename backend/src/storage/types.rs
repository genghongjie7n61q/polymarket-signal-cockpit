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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewRuntimeEvent {
    pub component: String,
    pub severity: String,
    pub event_type: String,
    pub message: String,
    pub details: Value,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReplayTick {
    pub market_key: String,
    pub window_start: OffsetDateTime,
    pub window_end: OffsetDateTime,
    pub source_ts: OffsetDateTime,
    pub price: BigDecimal,
    pub size: Option<BigDecimal>,
}
