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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReplayWindowCandidate {
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
pub struct ReplayWindow {
    pub market_key: String,
    pub window_start: OffsetDateTime,
    pub window_end: OffsetDateTime,
    pub open_price: BigDecimal,
    pub final_price: BigDecimal,
    pub candidates: Vec<ReplayWindowCandidate>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BacktestReplayResult {
    pub metrics: BacktestMetrics,
    pub replay_outputs: Vec<BacktestReplayOutput>,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BacktestEligibilityPolicy {
    pub min_trades: u64,
    pub min_wilson_lower_bound: BigDecimal,
    pub min_coverage: BigDecimal,
    pub min_expected_value: BigDecimal,
    pub max_drawdown: BigDecimal,
    pub max_consecutive_losses: u64,
    pub max_average_price_paid: BigDecimal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BacktestEligibility {
    pub eligible: bool,
    pub reasons: Vec<String>,
}

impl BacktestMetrics {
    pub fn production_eligibility(
        &self,
        policy: &BacktestEligibilityPolicy,
    ) -> BacktestEligibility {
        let mut reasons = Vec::new();
        if self.trades < policy.min_trades {
            reasons.push(format!(
                "min_trades not met: {} < {}",
                self.trades, policy.min_trades
            ));
        }
        if self.wilson_lower_bound < policy.min_wilson_lower_bound {
            reasons.push(format!(
                "min_wilson_lower_bound not met: {} < {}",
                self.wilson_lower_bound, policy.min_wilson_lower_bound
            ));
        }
        if self.coverage < policy.min_coverage {
            reasons.push(format!(
                "min_coverage not met: {} < {}",
                self.coverage, policy.min_coverage
            ));
        }
        if self.expected_value < policy.min_expected_value {
            reasons.push(format!(
                "min_expected_value not met: {} < {}",
                self.expected_value, policy.min_expected_value
            ));
        }
        if self.max_drawdown > policy.max_drawdown {
            reasons.push(format!(
                "max_drawdown exceeded: {} > {}",
                self.max_drawdown, policy.max_drawdown
            ));
        }
        if self.max_consecutive_losses > policy.max_consecutive_losses {
            reasons.push(format!(
                "max_consecutive_losses exceeded: {} > {}",
                self.max_consecutive_losses, policy.max_consecutive_losses
            ));
        }
        if self.average_price_paid > policy.max_average_price_paid {
            reasons.push(format!(
                "max_average_price_paid exceeded: {} > {}",
                self.average_price_paid, policy.max_average_price_paid
            ));
        }

        BacktestEligibility {
            eligible: reasons.is_empty(),
            reasons,
        }
    }
}
