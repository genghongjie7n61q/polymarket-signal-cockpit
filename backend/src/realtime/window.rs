use serde::{Deserialize, Serialize};
use time::{Duration, OffsetDateTime};

use crate::realtime::MarketKey;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketWindowState {
    pub market_key: MarketKey,
    pub event_slug: String,
    pub start_ts: OffsetDateTime,
    pub end_ts: OffsetDateTime,
}

pub fn window_for_tick(market_key: MarketKey, tick_ts: OffsetDateTime) -> MarketWindowState {
    let interval_seconds = market_key.interval_seconds();
    let unix_seconds = tick_ts.unix_timestamp();
    let start_seconds = unix_seconds - unix_seconds.rem_euclid(interval_seconds);
    let start_ts = OffsetDateTime::UNIX_EPOCH + Duration::seconds(start_seconds);
    let end_ts = start_ts + Duration::seconds(interval_seconds);

    MarketWindowState {
        market_key,
        event_slug: event_slug_for_window(market_key, start_seconds),
        start_ts,
        end_ts,
    }
}

fn event_slug_for_window(market_key: MarketKey, start_seconds: i64) -> String {
    match market_key {
        MarketKey::Btc5m => format!("btc-updown-5m-{start_seconds}"),
        MarketKey::Eth15m => format!("eth-updown-15m-{start_seconds}"),
    }
}
