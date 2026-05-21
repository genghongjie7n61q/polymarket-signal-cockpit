use std::{
    collections::BTreeMap,
    sync::{Arc, RwLock},
};

use bigdecimal::BigDecimal;
use serde::{Deserialize, Serialize};
use time::{Duration, OffsetDateTime};
use tokio::{sync::mpsc, task::JoinHandle};

use crate::realtime::{
    window_for_tick, CandleSnapshot, MarketKey, MarketTick, MarketWindowState, PolymarketSnapshot,
    RealtimeEvent,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateOwnerConfig {
    pub stale_after: Duration,
}

impl Default for StateOwnerConfig {
    fn default() -> Self {
        Self {
            stale_after: Duration::seconds(30),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealtimeStateMetrics {
    pub processed: u64,
    pub fanout_dropped: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LiveMarketState {
    pub market_key: MarketKey,
    pub latest_tick: Option<MarketTick>,
    pub current_window: Option<MarketWindowState>,
    pub latest_snapshot: Option<PolymarketSnapshot>,
    pub candles: Vec<CandleSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceSnapshot {
    pub last_seen_at: OffsetDateTime,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RealtimeStateSnapshot {
    pub markets: BTreeMap<MarketKey, LiveMarketState>,
    pub sources: BTreeMap<String, SourceSnapshot>,
    pub metrics: RealtimeStateMetrics,
}

#[derive(Clone)]
pub struct RealtimeStateOwner {
    state: Arc<RwLock<RealtimeStateSnapshot>>,
    config: StateOwnerConfig,
    _task: Arc<JoinHandle<()>>,
}

impl RealtimeStateOwner {
    pub fn spawn(
        mut rx: mpsc::Receiver<RealtimeEvent>,
        config: StateOwnerConfig,
        fanouts: Vec<mpsc::Sender<RealtimeEvent>>,
    ) -> Self {
        let state = Arc::new(RwLock::new(RealtimeStateSnapshot::default()));
        let task_state = state.clone();
        let task = tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                {
                    let mut state = task_state.write().expect("realtime state lock poisoned");
                    apply_event(&mut state, &event);
                    state.metrics.processed += 1;
                }

                let mut dropped = 0_u64;
                for fanout in &fanouts {
                    if fanout.try_send(event.clone()).is_err() {
                        dropped += 1;
                    }
                }

                if dropped > 0 {
                    let mut state = task_state.write().expect("realtime state lock poisoned");
                    state.metrics.fanout_dropped += dropped;
                }
            }
        });

        Self {
            state,
            config,
            _task: Arc::new(task),
        }
    }

    pub fn snapshot(&self) -> RealtimeStateSnapshot {
        self.state
            .read()
            .expect("realtime state lock poisoned")
            .clone()
    }

    pub fn source_status_at(&self, now: OffsetDateTime) -> BTreeMap<String, String> {
        self.snapshot()
            .sources
            .into_iter()
            .map(|(source, status)| {
                let state = if now - status.last_seen_at > self.config.stale_after {
                    "stale"
                } else {
                    "fresh"
                };
                (source, state.to_string())
            })
            .collect()
    }
}

fn apply_event(state: &mut RealtimeStateSnapshot, event: &RealtimeEvent) {
    match event {
        RealtimeEvent::Tick(tick) => {
            apply_tick(state, tick);
            mark_source_seen(state, &tick.source, tick.received_at);
        }
        RealtimeEvent::PolymarketSnapshot(snapshot) => {
            state
                .markets
                .entry(snapshot.market_key)
                .or_insert_with(|| empty_market(snapshot.market_key))
                .latest_snapshot = Some(snapshot.clone());
            mark_source_seen(state, "polymarket", snapshot.captured_at);
        }
        RealtimeEvent::SourceHeartbeat {
            source,
            received_at,
        } => mark_source_seen(state, source, *received_at),
    }
}

fn apply_tick(state: &mut RealtimeStateSnapshot, tick: &MarketTick) {
    let current_window = window_for_tick(tick.market_key, tick.source_ts);
    let market = state
        .markets
        .entry(tick.market_key)
        .or_insert_with(|| empty_market(tick.market_key));
    market.latest_tick = Some(tick.clone());
    market.current_window = Some(current_window);
    apply_tick_to_candles(&mut market.candles, tick);
}

fn apply_tick_to_candles(candles: &mut Vec<CandleSnapshot>, tick: &MarketTick) {
    let start_seconds =
        tick.source_ts.unix_timestamp() - tick.source_ts.unix_timestamp().rem_euclid(60);
    let start_ts = OffsetDateTime::UNIX_EPOCH + Duration::seconds(start_seconds);
    let volume = tick.size.clone().unwrap_or_else(|| BigDecimal::from(0));

    if let Some(candle) = candles
        .iter_mut()
        .find(|candle| candle.start_ts == start_ts)
    {
        if tick.price > candle.high {
            candle.high = tick.price.clone();
        }
        if tick.price < candle.low {
            candle.low = tick.price.clone();
        }
        candle.close = tick.price.clone();
        candle.volume += volume;
    } else {
        candles.push(CandleSnapshot {
            start_ts,
            open: tick.price.clone(),
            high: tick.price.clone(),
            low: tick.price.clone(),
            close: tick.price.clone(),
            volume,
        });
        candles.sort_by_key(|candle| candle.start_ts);
    }
}

fn mark_source_seen(state: &mut RealtimeStateSnapshot, source: &str, last_seen_at: OffsetDateTime) {
    state
        .sources
        .insert(source.to_string(), SourceSnapshot { last_seen_at });
}

fn empty_market(market_key: MarketKey) -> LiveMarketState {
    LiveMarketState {
        market_key,
        latest_tick: None,
        current_window: None,
        latest_snapshot: None,
        candles: Vec::new(),
    }
}
