use std::collections::BTreeMap;

use serde::Serialize;
use time::OffsetDateTime;

use crate::realtime::{
    RealtimeBus, RealtimeBusSnapshot, RealtimeEvent, RealtimeStateMetrics, RealtimeStateOwner,
    StateOwnerConfig,
};

pub const DEFAULT_REALTIME_QUEUE_CAPACITY: usize = 4096;

#[derive(Clone)]
pub struct RealtimeRuntime {
    bus: RealtimeBus,
    state_owner: RealtimeStateOwner,
}

impl RealtimeRuntime {
    pub fn spawn_default() -> Self {
        Self::spawn(DEFAULT_REALTIME_QUEUE_CAPACITY)
    }

    pub fn spawn(capacity: usize) -> Self {
        let (bus, rx) = RealtimeBus::bounded(capacity);
        let state_owner = RealtimeStateOwner::spawn(rx, StateOwnerConfig::default(), Vec::new());
        Self { bus, state_owner }
    }

    pub fn new(bus: RealtimeBus, state_owner: RealtimeStateOwner) -> Self {
        Self { bus, state_owner }
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
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RealtimeRuntimeSnapshot {
    pub bus: RealtimeBusSnapshot,
    pub state: RealtimeStateHealthSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RealtimeStateHealthSnapshot {
    pub metrics: RealtimeStateMetrics,
    pub sources: BTreeMap<String, String>,
    pub markets_tracked: usize,
}
