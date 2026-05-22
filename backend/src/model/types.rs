use bigdecimal::BigDecimal;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use time::OffsetDateTime;

use crate::realtime::MarketKey;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelAssignment {
    pub model_key: String,
    pub model_version: String,
    pub parameters: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelWindow {
    pub event_slug: String,
    pub start_ts: OffsetDateTime,
    pub end_ts: OffsetDateTime,
    pub elapsed_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelTickInput {
    pub source: String,
    pub source_ts: OffsetDateTime,
    pub received_at: OffsetDateTime,
    pub price: BigDecimal,
    pub size: Option<BigDecimal>,
    pub sequence: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelCandle {
    pub start_ts: OffsetDateTime,
    pub open: BigDecimal,
    pub high: BigDecimal,
    pub low: BigDecimal,
    pub close: BigDecimal,
    pub volume: BigDecimal,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelPolymarket {
    pub event_slug: String,
    pub up_price: Option<BigDecimal>,
    pub down_price: Option<BigDecimal>,
    pub spread: Option<BigDecimal>,
    pub liquidity: Option<BigDecimal>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelPortfolio {
    pub bankroll: BigDecimal,
    pub max_signal_size: BigDecimal,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelContext {
    pub market_key: MarketKey,
    pub symbol: String,
    pub assignment: ModelAssignment,
    pub window: ModelWindow,
    pub latest_tick: ModelTickInput,
    pub candles: Vec<ModelCandle>,
    pub polymarket: ModelPolymarket,
    pub portfolio: ModelPortfolio,
}

impl ModelContext {
    pub fn input_snapshot_hash(&self) -> String {
        let payload = serde_json::to_vec(self).expect("model context should serialize");
        let digest = Sha256::digest(payload);
        digest.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ModelAction {
    NoTrade,
    Candidate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelSide {
    Up,
    Down,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelCandidate {
    pub side: ModelSide,
    pub confidence: BigDecimal,
    pub limit_price: BigDecimal,
    pub suggested_size: BigDecimal,
    pub ttl_ms: i32,
    pub reason: String,
    pub features: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelDecision {
    pub action: ModelAction,
    pub side: Option<ModelSide>,
    pub confidence: Option<BigDecimal>,
    pub limit_price: Option<BigDecimal>,
    pub suggested_size: Option<BigDecimal>,
    pub ttl_ms: Option<i32>,
    pub reason: String,
    pub features: Value,
}

impl ModelDecision {
    pub fn no_trade(reason: impl Into<String>) -> Self {
        Self {
            action: ModelAction::NoTrade,
            side: None,
            confidence: None,
            limit_price: None,
            suggested_size: None,
            ttl_ms: None,
            reason: reason.into(),
            features: json!({}),
        }
    }

    pub fn candidate(candidate: ModelCandidate) -> Self {
        Self {
            action: ModelAction::Candidate,
            side: Some(candidate.side),
            confidence: Some(candidate.confidence),
            limit_price: Some(candidate.limit_price),
            suggested_size: Some(candidate.suggested_size),
            ttl_ms: Some(candidate.ttl_ms),
            reason: candidate.reason,
            features: candidate.features,
        }
    }

    pub fn ttl_ms(&self) -> Option<i32> {
        self.ttl_ms
    }

    pub fn features(&self) -> &Value {
        &self.features
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BacktestResult {
    pub model_key: String,
    pub model_version: String,
    pub dataset: String,
    pub trades: u64,
    pub wins: u64,
    pub accuracy: BigDecimal,
    pub wilson_lower_bound: BigDecimal,
    pub coverage: BigDecimal,
    pub max_drawdown: BigDecimal,
    pub parameters: Value,
}

pub trait StrategyModel: Send + Sync {
    fn key(&self) -> &'static str;
    fn version(&self) -> &'static str;
    fn decide(&self, context: &ModelContext) -> ModelDecision;
}
