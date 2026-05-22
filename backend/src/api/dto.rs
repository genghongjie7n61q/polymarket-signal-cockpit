use serde::Serialize;

use crate::realtime::{
    CandleSnapshot, MarketTick, MarketWindowState, PolymarketSnapshot, RealtimeRuntimeSnapshot,
};
use crate::storage::{CandleRecord, SignalWithMarketRecord, StorageWriterSnapshot};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MarketsResponseDto {
    pub markets: Vec<MarketSummaryDto>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MarketSummaryDto {
    pub market_key: String,
    pub symbol: String,
    pub interval_seconds: i64,
    pub source_status: Option<String>,
    pub current_window: Option<MarketWindowState>,
    pub latest_tick: Option<MarketTickDto>,
    pub latest_snapshot: Option<PolymarketSnapshotDto>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MarketStateDto {
    pub market_key: String,
    pub latest_tick: Option<MarketTickDto>,
    pub current_window: Option<MarketWindowState>,
    pub latest_snapshot: Option<PolymarketSnapshotDto>,
    pub recent_candles: Vec<CandleSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CandlesResponseDto {
    pub market_key: String,
    pub candles: Vec<CandleDto>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CandleDto {
    pub start_ts: time::OffsetDateTime,
    pub open: String,
    pub high: String,
    pub low: String,
    pub close: String,
    pub volume: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SignalsResponseDto {
    pub market_key: String,
    pub signals: Vec<SignalDto>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SignalDto {
    pub id: uuid::Uuid,
    pub market_window_id: uuid::Uuid,
    pub model_version_id: uuid::Uuid,
    pub signal_type: String,
    pub side: Option<String>,
    pub confidence: Option<String>,
    pub limit_price: Option<String>,
    pub suggested_size: Option<String>,
    pub ttl_ms: Option<i32>,
    pub reason: String,
    pub features: serde_json::Value,
    pub input_snapshot_hash: String,
    pub created_at: time::OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MarketTickDto {
    pub symbol: String,
    pub source: String,
    pub source_ts: time::OffsetDateTime,
    pub received_at: time::OffsetDateTime,
    pub price: String,
    pub size: Option<String>,
    pub sequence: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PolymarketSnapshotDto {
    pub event_slug: String,
    pub captured_at: time::OffsetDateTime,
    pub up_price: Option<String>,
    pub down_price: Option<String>,
    pub spread: Option<String>,
    pub liquidity: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RuntimeHealthDto {
    pub storage_writer: Option<StorageWriterSnapshot>,
    pub realtime: Option<RealtimeRuntimeSnapshot>,
}

impl MarketTickDto {
    pub fn from_tick(tick: &MarketTick) -> Self {
        Self {
            symbol: tick.symbol.clone(),
            source: tick.source.clone(),
            source_ts: tick.source_ts,
            received_at: tick.received_at,
            price: tick.price.to_string(),
            size: tick.size.as_ref().map(ToString::to_string),
            sequence: tick.sequence,
        }
    }
}

impl PolymarketSnapshotDto {
    pub fn from_snapshot(snapshot: &PolymarketSnapshot) -> Self {
        Self {
            event_slug: snapshot.event_slug.clone(),
            captured_at: snapshot.captured_at,
            up_price: snapshot.up_price.as_ref().map(ToString::to_string),
            down_price: snapshot.down_price.as_ref().map(ToString::to_string),
            spread: snapshot.spread.as_ref().map(ToString::to_string),
            liquidity: snapshot.liquidity.as_ref().map(ToString::to_string),
        }
    }
}

impl CandleDto {
    pub fn from_record(record: CandleRecord) -> Self {
        Self {
            start_ts: record.start_ts,
            open: record.open.to_string(),
            high: record.high.to_string(),
            low: record.low.to_string(),
            close: record.close.to_string(),
            volume: record.volume.to_string(),
        }
    }
}

impl SignalDto {
    pub fn from_record(record: SignalWithMarketRecord) -> Self {
        Self {
            id: record.id,
            market_window_id: record.market_window_id,
            model_version_id: record.model_version_id,
            signal_type: record.signal_type,
            side: record.side,
            confidence: record.confidence.map(|value| value.to_string()),
            limit_price: record.limit_price.map(|value| value.to_string()),
            suggested_size: record.suggested_size.map(|value| value.to_string()),
            ttl_ms: record.ttl_ms,
            reason: record.reason,
            features: record.features,
            input_snapshot_hash: record.input_snapshot_hash,
            created_at: record.created_at,
        }
    }
}
