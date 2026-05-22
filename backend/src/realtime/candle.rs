use std::collections::BTreeMap;

use bigdecimal::BigDecimal;
use serde::{Deserialize, Serialize};
use time::{Duration, OffsetDateTime};

use crate::realtime::MarketTick;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CandleSnapshot {
    pub start_ts: OffsetDateTime,
    pub open: BigDecimal,
    pub high: BigDecimal,
    pub low: BigDecimal,
    pub close: BigDecimal,
    pub volume: BigDecimal,
}

#[derive(Debug, Default, Clone)]
pub struct CandleAggregator {
    candles: BTreeMap<i64, CandleSnapshot>,
}

impl CandleAggregator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn apply_tick(&mut self, tick: &MarketTick) {
        let start_seconds = minute_start_seconds(tick.source_ts);
        let volume = tick.size.clone().unwrap_or_else(|| BigDecimal::from(0));

        self.candles
            .entry(start_seconds)
            .and_modify(|candle| {
                if tick.price > candle.high {
                    candle.high = tick.price.clone();
                }
                if tick.price < candle.low {
                    candle.low = tick.price.clone();
                }
                candle.close = tick.price.clone();
                candle.volume += volume.clone();
            })
            .or_insert_with(|| CandleSnapshot {
                start_ts: OffsetDateTime::UNIX_EPOCH + Duration::seconds(start_seconds),
                open: tick.price.clone(),
                high: tick.price.clone(),
                low: tick.price.clone(),
                close: tick.price.clone(),
                volume,
            });
    }

    pub fn snapshots(&self) -> Vec<CandleSnapshot> {
        self.candles.values().cloned().collect()
    }
}

fn minute_start_seconds(ts: OffsetDateTime) -> i64 {
    let unix_seconds = ts.unix_timestamp();
    unix_seconds - unix_seconds.rem_euclid(60)
}
