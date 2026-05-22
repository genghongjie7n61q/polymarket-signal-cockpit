use std::collections::BTreeMap;

use serde::Serialize;
use time::OffsetDateTime;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::realtime::{
    LiveMarketSummary, MarketKey, RealtimeBus, RealtimeBusSnapshot, RealtimeEvent,
    RealtimeStateMetrics, RealtimeStateOwner, RealtimeStateSnapshot, RealtimeStorageBridge,
    RealtimeStorageBridgeSnapshot, StateOwnerConfig,
};
use crate::storage::StorageWriterHandle;

pub const DEFAULT_REALTIME_QUEUE_CAPACITY: usize = 4096;

#[derive(Clone)]
pub struct RealtimeRuntime {
    bus: RealtimeBus,
    state_owner: RealtimeStateOwner,
    storage_bridge: Option<RealtimeStorageBridge>,
}

impl RealtimeRuntime {
    pub fn spawn_default() -> Self {
        Self::spawn(DEFAULT_REALTIME_QUEUE_CAPACITY)
    }

    pub fn spawn(capacity: usize) -> Self {
        let (bus, rx) = RealtimeBus::bounded(capacity);
        let state_owner = RealtimeStateOwner::spawn(rx, StateOwnerConfig::default(), Vec::new());
        Self {
            bus,
            state_owner,
            storage_bridge: None,
        }
    }

    pub fn spawn_with_storage(
        capacity: usize,
        writer: StorageWriterHandle,
        market_ids: BTreeMap<MarketKey, Uuid>,
    ) -> Self {
        let (bus, rx) = RealtimeBus::bounded(capacity);
        let (storage_tx, storage_rx) = mpsc::channel(capacity);
        let storage_bridge = RealtimeStorageBridge::spawn(storage_rx, writer, market_ids);
        let state_owner =
            RealtimeStateOwner::spawn(rx, StateOwnerConfig::default(), vec![storage_tx]);
        Self {
            bus,
            state_owner,
            storage_bridge: Some(storage_bridge),
        }
    }

    pub fn new(bus: RealtimeBus, state_owner: RealtimeStateOwner) -> Self {
        Self {
            bus,
            state_owner,
            storage_bridge: None,
        }
    }

    pub fn bus(&self) -> &RealtimeBus {
        &self.bus
    }

    pub fn publish(&self, event: RealtimeEvent) -> Result<(), crate::realtime::RealtimeError> {
        self.bus.try_publish(event)
    }

    pub fn snapshot(&self, now: OffsetDateTime) -> RealtimeRuntimeSnapshot {
        let state = self.state_owner.snapshot();
        RealtimeRuntimeSnapshot {
            bus: self.bus.snapshot(),
            state: RealtimeStateHealthSnapshot {
                metrics: state.metrics,
                sources: self.state_owner.source_status_at(now),
                markets_tracked: state.markets.len(),
            },
            storage_bridge: self
                .storage_bridge
                .as_ref()
                .map(RealtimeStorageBridge::snapshot),
        }
    }

    pub fn state_snapshot(&self) -> RealtimeStateSnapshot {
        self.state_owner.snapshot()
    }

    pub fn source_status_at(&self, now: OffsetDateTime) -> BTreeMap<String, String> {
        self.state_owner.source_status_at(now)
    }

    pub fn market_summaries_at(
        &self,
        market_keys: &[MarketKey],
        now: OffsetDateTime,
    ) -> Vec<LiveMarketSummary> {
        self.state_owner.market_summaries_at(market_keys, now)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RealtimeRuntimeSnapshot {
    pub bus: RealtimeBusSnapshot,
    pub state: RealtimeStateHealthSnapshot,
    pub storage_bridge: Option<RealtimeStorageBridgeSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RealtimeStateHealthSnapshot {
    pub metrics: RealtimeStateMetrics,
    pub sources: BTreeMap<String, String>,
    pub markets_tracked: usize,
}
