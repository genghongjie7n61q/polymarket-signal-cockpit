use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::OffsetDateTime;

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
