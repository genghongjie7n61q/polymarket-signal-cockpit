use bigdecimal::BigDecimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::OffsetDateTime;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FrozenActionableAlert {
    pub created_at: OffsetDateTime,
    pub side: String,
    pub confidence: BigDecimal,
    pub limit_price: BigDecimal,
    pub suggested_size: BigDecimal,
    pub ttl_ms: i32,
    pub reason: String,
    pub features: Value,
    pub input_snapshot_hash: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BacktestReplayOutput {
    pub market_key: String,
    pub window_start: OffsetDateTime,
    pub window_end: OffsetDateTime,
    pub alert: FrozenActionableAlert,
    pub open_price: BigDecimal,
    pub final_price: BigDecimal,
    pub won: bool,
    pub profit_loss: BigDecimal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalibrationBucket {
    pub lower_bound: BigDecimal,
    pub upper_bound: BigDecimal,
    pub trades: u64,
    pub wins: u64,
    pub accuracy: BigDecimal,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BacktestMetrics {
    pub model_key: String,
    pub model_version: String,
    pub market_key: String,
    pub dataset: String,
    pub windows: u64,
    pub trades: u64,
    pub wins: u64,
    pub accuracy: BigDecimal,
    pub wilson_lower_bound: BigDecimal,
    pub coverage: BigDecimal,
    pub expected_value: BigDecimal,
    pub max_drawdown: BigDecimal,
    pub max_consecutive_losses: u64,
    pub average_price_paid: BigDecimal,
    pub calibration_buckets: Vec<CalibrationBucket>,
    pub parameters: Value,
}
