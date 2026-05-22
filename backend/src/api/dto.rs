use serde::Serialize;

use crate::realtime::{
    CandleSnapshot, MarketTick, MarketWindowState, PolymarketSnapshot, RealtimeRuntimeSnapshot,
};
use crate::storage::StorageWriterSnapshot;

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
