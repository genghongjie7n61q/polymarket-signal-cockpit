use bigdecimal::BigDecimal;
use serde_json::json;

use crate::model::{
    ModelCandidate, ModelContext, ModelDecision, ModelSide, SizingConfig, SizingEngine,
    StrategyModel,
};

pub const BASELINE_DIRECTION_KEY: &str = "baseline_direction";
pub const BASELINE_DIRECTION_VERSION: &str = "0.1.0";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaselineDirectionConfig {
    pub decision_offset_ms: i64,
    pub threshold_bps: BigDecimal,
    pub ttl_ms: i32,
    pub sizing: SizingConfig,
}

impl Default for BaselineDirectionConfig {
    fn default() -> Self {
        Self {
            decision_offset_ms: 180_000,
            threshold_bps: BigDecimal::from(4),
            ttl_ms: 15_000,
            sizing: SizingConfig {
                bankroll: BigDecimal::from(15),
                fraction: BigDecimal::from(1) / BigDecimal::from(2),
                max_size: BigDecimal::from(3) / BigDecimal::from(2),
                min_size: BigDecimal::from(1) / BigDecimal::from(2),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaselineDirectionModel {
    config: BaselineDirectionConfig,
}

impl BaselineDirectionModel {
    pub fn new(config: BaselineDirectionConfig) -> Self {
        Self { config }
    }

    pub fn btc5m_default() -> Self {
        Self::new(BaselineDirectionConfig::default())
    }

    pub fn eth15m_default() -> Self {
        Self::new(BaselineDirectionConfig::default())
    }
}

impl StrategyModel for BaselineDirectionModel {
    fn key(&self) -> &'static str {
        BASELINE_DIRECTION_KEY
    }

    fn version(&self) -> &'static str {
        BASELINE_DIRECTION_VERSION
    }

    fn decide(&self, context: &ModelContext) -> ModelDecision {
        if context.window.elapsed_ms < self.config.decision_offset_ms {
            return ModelDecision::no_trade("waiting for decision offset");
        }

        let Some(open_price) = context.candles.first().map(|candle| &candle.open) else {
            return ModelDecision::no_trade("missing opening candle");
        };
        if open_price <= &BigDecimal::from(0) {
            return ModelDecision::no_trade("invalid opening price");
        }

        let return_bps =
            ((&context.latest_tick.price - open_price) / open_price) * BigDecimal::from(10_000);
        let abs_return_bps = abs_decimal(&return_bps);
        if abs_return_bps < self.config.threshold_bps {
            return ModelDecision::no_trade("move below threshold");
        }

        let side = if return_bps >= BigDecimal::from(0) {
            ModelSide::Up
        } else {
            ModelSide::Down
        };
        let Some(limit_price) = limit_price_for_side(context, side) else {
            return ModelDecision::no_trade("missing polymarket side price");
        };

        let confidence = cap_decimal(
            BigDecimal::from(1) / BigDecimal::from(2)
                + abs_return_bps.clone() / BigDecimal::from(10_000),
            BigDecimal::from(99) / BigDecimal::from(100),
        );
        let sizing = SizingEngine::new(sizing_config_for_context(&self.config, context));
        let Some(suggested_size) = sizing.suggest_size(&confidence, limit_price) else {
            return ModelDecision::no_trade("positive edge below minimum size");
        };

        let input_snapshot_hash = context.input_snapshot_hash();
        ModelDecision::candidate(ModelCandidate {
            side,
            confidence: confidence.clone(),
            limit_price: limit_price.clone(),
            suggested_size,
            ttl_ms: self.config.ttl_ms,
            reason: "directional threshold crossed".to_string(),
            features: json!({
                "model_key": BASELINE_DIRECTION_KEY,
                "model_version": BASELINE_DIRECTION_VERSION,
                "market_key": context.market_key.as_str(),
                "return_bps": decimal_string(&return_bps),
                "abs_return_bps": decimal_string(&abs_return_bps),
                "threshold_bps": decimal_string(&self.config.threshold_bps),
                "elapsed_ms": context.window.elapsed_ms,
                "limit_price": decimal_string(limit_price),
                "confidence": decimal_string(&confidence),
                "input_snapshot_hash": input_snapshot_hash,
            }),
        })
    }
}

fn limit_price_for_side(context: &ModelContext, side: ModelSide) -> Option<&BigDecimal> {
    match side {
        ModelSide::Up => context.polymarket.up_price.as_ref(),
        ModelSide::Down => context.polymarket.down_price.as_ref(),
    }
}

fn abs_decimal(value: &BigDecimal) -> BigDecimal {
    if value < &BigDecimal::from(0) {
        -value.clone()
    } else {
        value.clone()
    }
}

fn cap_decimal(value: BigDecimal, max: BigDecimal) -> BigDecimal {
    if value > max {
        max
    } else {
        value
    }
}

fn min_decimal(left: BigDecimal, right: BigDecimal) -> BigDecimal {
    if left < right {
        left
    } else {
        right
    }
}

fn sizing_config_for_context(
    config: &BaselineDirectionConfig,
    context: &ModelContext,
) -> SizingConfig {
    SizingConfig {
        bankroll: context.portfolio.bankroll.clone(),
        fraction: config.sizing.fraction.clone(),
        max_size: min_decimal(
            config.sizing.max_size.clone(),
            context.portfolio.max_signal_size.clone(),
        ),
        min_size: config.sizing.min_size.clone(),
    }
}

fn decimal_string(value: &BigDecimal) -> String {
    let mut text = value.to_string();
    if text.contains('.') {
        while text.ends_with('0') {
            text.pop();
        }
        if text.ends_with('.') {
            text.pop();
        }
    }
    text
}
