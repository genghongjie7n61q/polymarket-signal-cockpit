use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
};

use serde::Serialize;
use tokio::{sync::mpsc, task::JoinHandle};
use uuid::Uuid;

use crate::{
    realtime::{MarketKey, MarketTick, RealtimeEvent},
    storage::{NewTick, StorageCommand, StorageWriterHandle},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RealtimeStorageBridgeSnapshot {
    pub processed: u64,
    pub enqueued: u64,
    pub dropped: u64,
    pub missing_market: u64,
    pub ignored: u64,
}

#[derive(Clone)]
pub struct RealtimeStorageBridge {
    metrics: Arc<RealtimeStorageBridgeMetrics>,
    _task: Arc<Mutex<Option<JoinHandle<()>>>>,
}

impl RealtimeStorageBridge {
    pub fn spawn(
        mut rx: mpsc::Receiver<RealtimeEvent>,
        writer: StorageWriterHandle,
        market_ids: BTreeMap<MarketKey, Uuid>,
    ) -> Self {
        let metrics = Arc::new(RealtimeStorageBridgeMetrics::default());
        let task_metrics = metrics.clone();
        let task = tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                task_metrics.processed.fetch_add(1, Ordering::Relaxed);
                match event {
                    RealtimeEvent::Tick(tick) => {
                        enqueue_tick(&writer, &market_ids, tick, &task_metrics);
                    }
                    RealtimeEvent::PolymarketSnapshot(_)
                    | RealtimeEvent::SourceHeartbeat { .. } => {
                        task_metrics.ignored.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
        });

        Self {
            metrics,
            _task: Arc::new(Mutex::new(Some(task))),
        }
    }

    pub fn snapshot(&self) -> RealtimeStorageBridgeSnapshot {
        RealtimeStorageBridgeSnapshot {
            processed: self.metrics.processed.load(Ordering::Relaxed),
            enqueued: self.metrics.enqueued.load(Ordering::Relaxed),
            dropped: self.metrics.dropped.load(Ordering::Relaxed),
            missing_market: self.metrics.missing_market.load(Ordering::Relaxed),
            ignored: self.metrics.ignored.load(Ordering::Relaxed),
        }
    }
}

#[derive(Default)]
struct RealtimeStorageBridgeMetrics {
    processed: AtomicU64,
    enqueued: AtomicU64,
    dropped: AtomicU64,
    missing_market: AtomicU64,
    ignored: AtomicU64,
}

fn enqueue_tick(
    writer: &StorageWriterHandle,
    market_ids: &BTreeMap<MarketKey, Uuid>,
    tick: MarketTick,
    metrics: &RealtimeStorageBridgeMetrics,
) {
    let Some(market_id) = market_ids.get(&tick.market_key).copied() else {
        metrics.missing_market.fetch_add(1, Ordering::Relaxed);
        return;
    };

    let command = StorageCommand::Tick(NewTick {
        market_id,
        source: tick.source,
        source_ts: tick.source_ts,
        received_at: tick.received_at,
        price: tick.price,
        size: tick.size,
        sequence: tick.sequence,
    });

    match writer.try_enqueue(command) {
        Ok(()) => {
            metrics.enqueued.fetch_add(1, Ordering::Relaxed);
        }
        Err(error) => {
            metrics.dropped.fetch_add(1, Ordering::Relaxed);
            tracing::warn!(%error, "realtime storage bridge dropped event");
        }
    }
}
