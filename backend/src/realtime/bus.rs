use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use tokio::sync::mpsc;

use crate::realtime::{MarketTick, PolymarketSnapshot, RealtimeError};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RealtimeEvent {
    Tick(MarketTick),
    PolymarketSnapshot(PolymarketSnapshot),
    SourceHeartbeat {
        source: String,
        received_at: OffsetDateTime,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RealtimeBusSnapshot {
    pub queued_capacity: usize,
    pub queued_available: usize,
    pub accepted: u64,
    pub dropped: u64,
}

#[derive(Clone)]
pub struct RealtimeBus {
    tx: mpsc::Sender<RealtimeEvent>,
    metrics: Arc<RealtimeBusMetrics>,
    capacity: usize,
}

impl RealtimeBus {
    pub fn bounded(capacity: usize) -> (Self, mpsc::Receiver<RealtimeEvent>) {
        let (tx, rx) = mpsc::channel(capacity);
        (
            Self {
                tx,
                metrics: Arc::new(RealtimeBusMetrics::default()),
                capacity,
            },
            rx,
        )
    }

    pub fn try_publish(&self, event: RealtimeEvent) -> Result<(), RealtimeError> {
        match self.tx.try_send(event) {
            Ok(()) => {
                self.metrics.accepted.fetch_add(1, Ordering::Relaxed);
                Ok(())
            }
            Err(mpsc::error::TrySendError::Full(_)) => {
                self.metrics.dropped.fetch_add(1, Ordering::Relaxed);
                Err(RealtimeError::QueueFull)
            }
            Err(mpsc::error::TrySendError::Closed(_)) => Err(RealtimeError::QueueClosed),
        }
    }

    pub fn snapshot(&self) -> RealtimeBusSnapshot {
        RealtimeBusSnapshot {
            queued_capacity: self.capacity,
            queued_available: self.tx.capacity(),
            accepted: self.metrics.accepted.load(Ordering::Relaxed),
            dropped: self.metrics.dropped.load(Ordering::Relaxed),
        }
    }
}

#[derive(Default)]
struct RealtimeBusMetrics {
    accepted: AtomicU64,
    dropped: AtomicU64,
}
