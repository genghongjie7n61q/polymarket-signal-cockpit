use std::str::FromStr;

use bigdecimal::BigDecimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use time::OffsetDateTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MarketKey {
    Btc5m,
    Eth15m,
}

impl MarketKey {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Btc5m => "btc5m",
            Self::Eth15m => "eth15m",
        }
    }

    pub fn symbol(&self) -> &'static str {
        match self {
            Self::Btc5m => "BTC-USD",
            Self::Eth15m => "ETH-USD",
        }
    }

    pub fn interval_seconds(&self) -> i64 {
        match self {
            Self::Btc5m => 300,
            Self::Eth15m => 900,
        }
    }

    pub fn from_coinbase_product(product_id: &str) -> Result<Self, RealtimeError> {
        match product_id {
            "BTC-USD" => Ok(Self::Btc5m),
            "ETH-USD" => Ok(Self::Eth15m),
            other => Err(RealtimeError::UnsupportedProduct(other.to_string())),
        }
    }
}

impl FromStr for MarketKey {
    type Err = RealtimeError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "btc5m" => Ok(Self::Btc5m),
            "eth15m" => Ok(Self::Eth15m),
            other => Err(RealtimeError::UnsupportedMarket(other.to_string())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketTick {
    pub market_key: MarketKey,
    pub symbol: String,
    pub source: String,
    pub source_ts: OffsetDateTime,
    pub received_at: OffsetDateTime,
    pub price: BigDecimal,
    pub size: Option<BigDecimal>,
    pub sequence: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PolymarketSnapshot {
    pub market_key: MarketKey,
    pub event_slug: String,
    pub captured_at: OffsetDateTime,
    pub up_price: Option<BigDecimal>,
    pub down_price: Option<BigDecimal>,
    pub spread: Option<BigDecimal>,
    pub liquidity: Option<BigDecimal>,
    pub payload: Value,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RealtimeError {
    #[error("unsupported market: {0}")]
    UnsupportedMarket(String),
    #[error("unsupported Coinbase product: {0}")]
    UnsupportedProduct(String),
    #[error("missing field: {0}")]
    MissingField(&'static str),
    #[error("invalid decimal field {field}: {value}")]
    InvalidDecimal { field: &'static str, value: String },
    #[error("invalid timestamp field {field}: {value}")]
    InvalidTimestamp { field: &'static str, value: String },
    #[error("invalid integer field {field}: {value}")]
    InvalidInteger { field: &'static str, value: String },
    #[error("realtime queue is full")]
    QueueFull,
    #[error("realtime queue is closed")]
    QueueClosed,
}
