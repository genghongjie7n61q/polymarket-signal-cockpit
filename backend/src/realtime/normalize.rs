use std::str::FromStr;

use bigdecimal::BigDecimal;
use serde_json::Value;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

use crate::realtime::{MarketKey, MarketTick, PolymarketSnapshot, RealtimeError};

const COINBASE_SOURCE: &str = "coinbase";

pub fn normalize_coinbase_ticker(
    payload: &Value,
    received_at: OffsetDateTime,
) -> Result<MarketTick, RealtimeError> {
    let product_id = required_str(payload, "product_id")?;
    let market_key = MarketKey::from_coinbase_product(product_id)?;
    let source_ts = parse_rfc3339(required_str(payload, "time")?, "time")?;
    let price = parse_decimal(required_str(payload, "price")?, "price")?;
    let size = optional_decimal(payload, "last_size")?;
    let sequence = optional_i64(payload, "sequence")?;

    Ok(MarketTick {
        market_key,
        symbol: product_id.to_string(),
        source: COINBASE_SOURCE.to_string(),
        source_ts,
        received_at,
        price,
        size,
        sequence,
    })
}

pub fn normalize_polymarket_snapshot(
    market_key: MarketKey,
    payload: &Value,
    captured_at: OffsetDateTime,
) -> Result<PolymarketSnapshot, RealtimeError> {
    let event_slug = required_str(payload, "event_slug")?.to_string();
    let up_price = optional_decimal(payload, "up_price")?;
    let down_price = optional_decimal(payload, "down_price")?;
    let spread = match (&up_price, &down_price) {
        (Some(up), Some(down)) if up >= down => Some(up - down),
        (Some(up), Some(down)) => Some(down - up),
        _ => optional_decimal(payload, "spread")?,
    };
    let liquidity = optional_decimal(payload, "liquidity")?;

    Ok(PolymarketSnapshot {
        market_key,
        event_slug,
        captured_at,
        up_price,
        down_price,
        spread,
        liquidity,
        payload: payload.clone(),
    })
}

fn required_str<'a>(payload: &'a Value, field: &'static str) -> Result<&'a str, RealtimeError> {
    payload
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or(RealtimeError::MissingField(field))
}

fn optional_decimal(
    payload: &Value,
    field: &'static str,
) -> Result<Option<BigDecimal>, RealtimeError> {
    match payload.get(field) {
        Some(Value::String(value)) if !value.is_empty() => parse_decimal(value, field).map(Some),
        Some(Value::Number(value)) => parse_decimal(&value.to_string(), field).map(Some),
        Some(Value::Null) | None => Ok(None),
        Some(other) => Err(RealtimeError::InvalidDecimal {
            field,
            value: other.to_string(),
        }),
    }
}

fn parse_decimal(value: &str, field: &'static str) -> Result<BigDecimal, RealtimeError> {
    BigDecimal::from_str(value).map_err(|_| RealtimeError::InvalidDecimal {
        field,
        value: value.to_string(),
    })
}

fn optional_i64(payload: &Value, field: &'static str) -> Result<Option<i64>, RealtimeError> {
    match payload.get(field) {
        Some(Value::Number(value)) => value
            .as_i64()
            .ok_or_else(|| RealtimeError::InvalidInteger {
                field,
                value: value.to_string(),
            })
            .map(Some),
        Some(Value::String(value)) if !value.is_empty() => {
            value
                .parse::<i64>()
                .map(Some)
                .map_err(|_| RealtimeError::InvalidInteger {
                    field,
                    value: value.to_string(),
                })
        }
        Some(Value::Null) | None => Ok(None),
        Some(other) => Err(RealtimeError::InvalidInteger {
            field,
            value: other.to_string(),
        }),
    }
}

fn parse_rfc3339(value: &str, field: &'static str) -> Result<OffsetDateTime, RealtimeError> {
    OffsetDateTime::parse(value, &Rfc3339).map_err(|_| RealtimeError::InvalidTimestamp {
        field,
        value: value.to_string(),
    })
}
