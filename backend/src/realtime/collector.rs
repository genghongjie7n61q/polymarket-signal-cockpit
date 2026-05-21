use serde_json::Value;
use time::{Duration, OffsetDateTime};

use crate::realtime::{
    normalize_coinbase_ticker, normalize_polymarket_snapshot, MarketKey, RealtimeError,
    RealtimeEvent, RealtimeRuntime,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoinbaseCollectorConfig {
    pub symbols: Vec<String>,
}

impl Default for CoinbaseCollectorConfig {
    fn default() -> Self {
        Self {
            symbols: vec!["BTC-USD".to_string(), "ETH-USD".to_string()],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolymarketSnapshotRefresherConfig {
    pub markets: Vec<MarketKey>,
    pub interval: Duration,
}

impl Default for PolymarketSnapshotRefresherConfig {
    fn default() -> Self {
        Self {
            markets: vec![MarketKey::Btc5m, MarketKey::Eth15m],
            interval: Duration::seconds(15),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum CollectorEvent {
    CoinbaseTicker {
        payload: Value,
        received_at: OffsetDateTime,
    },
    PolymarketSnapshot {
        market_key: MarketKey,
        payload: Value,
        captured_at: OffsetDateTime,
    },
    Heartbeat {
        source: String,
        received_at: OffsetDateTime,
    },
}

pub trait CollectorEventSink: Clone + Send + Sync + 'static {
    fn try_publish(&self, event: RealtimeEvent) -> Result<(), RealtimeError>;
}

impl CollectorEventSink for RealtimeRuntime {
    fn try_publish(&self, event: RealtimeEvent) -> Result<(), RealtimeError> {
        self.publish(event)
    }
}

#[derive(Clone)]
pub struct CollectorIngress<S> {
    sink: S,
}

impl<S> CollectorIngress<S>
where
    S: CollectorEventSink,
{
    pub fn new(sink: S) -> Self {
        Self { sink }
    }

    pub fn handle(&self, event: CollectorEvent) -> Result<(), RealtimeError> {
        let event = match event {
            CollectorEvent::CoinbaseTicker {
                payload,
                received_at,
            } => RealtimeEvent::Tick(normalize_coinbase_ticker(&payload, received_at)?),
            CollectorEvent::PolymarketSnapshot {
                market_key,
                payload,
                captured_at,
            } => RealtimeEvent::PolymarketSnapshot(normalize_polymarket_snapshot(
                market_key,
                &payload,
                captured_at,
            )?),
            CollectorEvent::Heartbeat {
                source,
                received_at,
            } => RealtimeEvent::SourceHeartbeat {
                source,
                received_at,
            },
        };

        self.sink.try_publish(event)
    }
}
