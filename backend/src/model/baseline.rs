use bigdecimal::BigDecimal;
use serde_json::json;
use serde_json::Value;
use std::{fmt, str::FromStr};

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

impl BaselineDirectionConfig {
    pub fn from_assignment(context: &ModelContext, defaults: &Self) -> Result<Self, ConfigError> {
        let parameters = context
            .assignment
            .parameters
            .as_object()
            .ok_or(ConfigError::Message("parameters must be an object"))?;

        let mut config = defaults.clone();
        if let Some(value) = parameters.get("decision_offset_ms") {
            config.decision_offset_ms = parse_non_negative_i64("decision_offset_ms", value)?;
        }
        if let Some(value) = parameters.get("threshold_bps") {
            config.threshold_bps = parse_positive_decimal("threshold_bps", value)?;
        }
        if let Some(value) = parameters.get("ttl_ms") {
            config.ttl_ms = parse_positive_i32("ttl_ms", value)?;
        }
        if let Some(value) = parameters.get("kelly_fraction") {
            config.sizing.fraction = parse_fraction("kelly_fraction", value)?;
        }
        if let Some(value) = parameters.get("min_size") {
            config.sizing.min_size = parse_positive_decimal("min_size", value)?;
        }
        if let Some(value) = parameters.get("max_size") {
            config.sizing.max_size = parse_positive_decimal("max_size", value)?;
        }

        config.sizing.bankroll = context.portfolio.bankroll.clone();
        config.sizing.max_size = min_decimal(
            config.sizing.max_size,
            context.portfolio.max_signal_size.clone(),
        );
        if config.sizing.bankroll <= BigDecimal::from(0) {
            return Err(ConfigError::Message("bankroll must be positive"));
        }
        if config.sizing.max_size <= BigDecimal::from(0) {
            return Err(ConfigError::Message("max_size must be positive"));
        }
        if config.sizing.min_size > config.sizing.max_size {
            return Err(ConfigError::Message("min_size must be <= max_size"));
        }

        Ok(config)
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
        let config = match BaselineDirectionConfig::from_assignment(context, &self.config) {
            Ok(config) => config,
            Err(error) => {
                return ModelDecision::no_trade(format!("invalid assignment parameters: {error}"));
            }
        };

        if context.window.elapsed_ms < config.decision_offset_ms {
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
        if abs_return_bps < config.threshold_bps {
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
        let sizing = SizingEngine::new(config.sizing.clone());
        let Some(suggested_size) = sizing.suggest_size(&confidence, limit_price) else {
            return ModelDecision::no_trade("positive edge below minimum size");
        };

        let input_snapshot_hash = context.input_snapshot_hash();
        ModelDecision::candidate(ModelCandidate {
            side,
            confidence: confidence.clone(),
            limit_price: limit_price.clone(),
            suggested_size,
            ttl_ms: config.ttl_ms,
            reason: "directional threshold crossed".to_string(),
            features: json!({
                "model_key": BASELINE_DIRECTION_KEY,
                "model_version": BASELINE_DIRECTION_VERSION,
                "market_key": context.market_key.as_str(),
                "return_bps": decimal_string(&return_bps),
                "abs_return_bps": decimal_string(&abs_return_bps),
                "threshold_bps": decimal_string(&config.threshold_bps),
                "elapsed_ms": context.window.elapsed_ms,
                "limit_price": decimal_string(limit_price),
                "confidence": decimal_string(&confidence),
                "kelly_fraction": decimal_string(&config.sizing.fraction),
                "min_size": decimal_string(&config.sizing.min_size),
                "max_size": decimal_string(&config.sizing.max_size),
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    Message(&'static str),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Message(message) => formatter.write_str(message),
        }
    }
}

fn parse_non_negative_i64(field: &'static str, value: &Value) -> Result<i64, ConfigError> {
    let parsed = parse_i64(field, value)?;
    if parsed < 0 {
        return Err(ConfigError::Message(non_negative_message(field)));
    }
    Ok(parsed)
}

fn parse_positive_i32(field: &'static str, value: &Value) -> Result<i32, ConfigError> {
    let parsed = parse_i64(field, value)?;
    if parsed <= 0 {
        return Err(ConfigError::Message(positive_message(field)));
    }
    i32::try_from(parsed).map_err(|_| ConfigError::Message("ttl_ms is too large"))
}

fn parse_i64(field: &'static str, value: &Value) -> Result<i64, ConfigError> {
    match value {
        Value::Number(number) => number
            .as_i64()
            .ok_or(ConfigError::Message(integer_message(field))),
        Value::String(text) => text
            .parse::<i64>()
            .map_err(|_| ConfigError::Message(integer_message(field))),
        _ => Err(ConfigError::Message(integer_message(field))),
    }
}

fn parse_positive_decimal(field: &'static str, value: &Value) -> Result<BigDecimal, ConfigError> {
    let parsed = parse_decimal(field, value)?;
    if parsed <= BigDecimal::from(0) {
        return Err(ConfigError::Message(positive_message(field)));
    }
    Ok(parsed)
}

fn parse_fraction(field: &'static str, value: &Value) -> Result<BigDecimal, ConfigError> {
    let parsed = parse_positive_decimal(field, value)?;
    if parsed > BigDecimal::from(1) {
        return Err(ConfigError::Message("kelly_fraction must be <= 1"));
    }
    Ok(parsed)
}

fn parse_decimal(field: &'static str, value: &Value) -> Result<BigDecimal, ConfigError> {
    let text = match value {
        Value::Number(number) => number.to_string(),
        Value::String(text) => text.clone(),
        _ => return Err(ConfigError::Message(decimal_message(field))),
    };

    BigDecimal::from_str(&text).map_err(|_| ConfigError::Message(decimal_message(field)))
}

fn positive_message(field: &'static str) -> &'static str {
    match field {
        "threshold_bps" => "threshold_bps must be positive",
        "ttl_ms" => "ttl_ms must be positive",
        "min_size" => "min_size must be positive",
        "max_size" => "max_size must be positive",
        _ => "value must be positive",
    }
}

fn non_negative_message(field: &'static str) -> &'static str {
    match field {
        "decision_offset_ms" => "decision_offset_ms must be non-negative",
        _ => "value must be non-negative",
    }
}

fn integer_message(field: &'static str) -> &'static str {
    match field {
        "decision_offset_ms" => "decision_offset_ms must be an integer",
        "ttl_ms" => "ttl_ms must be an integer",
        _ => "value must be an integer",
    }
}

fn decimal_message(field: &'static str) -> &'static str {
    match field {
        "threshold_bps" => "threshold_bps must be a decimal",
        "kelly_fraction" => "kelly_fraction must be a decimal",
        "min_size" => "min_size must be a decimal",
        "max_size" => "max_size must be a decimal",
        _ => "value must be a decimal",
    }
}
