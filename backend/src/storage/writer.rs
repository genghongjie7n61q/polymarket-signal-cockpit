use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

use tokio::{sync::mpsc, task::JoinHandle, time};

use crate::storage::{
    NewNotificationDelivery, NewRawMarketEvent, NewRuntimeEvent, NewSignal, NewTick, StorageError,
    StorageRepository,
};

const MAX_WRITE_ATTEMPTS: u8 = 3;

#[derive(Debug, Clone)]
pub enum StorageCommand {
    RawMarketEvent(NewRawMarketEvent),
    Tick(NewTick),
    Signal(NewSignal),
    NotificationDelivery(NewNotificationDelivery),
    RuntimeEvent(NewRuntimeEvent),
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct StorageWriterSnapshot {
    pub queued_capacity: usize,
    pub queued_available: usize,
    pub task_status: &'static str,
    pub accepted: u64,
    pub dropped: u64,
    pub written: u64,
    pub failed: u64,
    pub retried: u64,
}

#[derive(Clone)]
pub struct StorageWriterHandle {
    tx: mpsc::Sender<StorageCommand>,
    metrics: Arc<StorageWriterMetrics>,
    capacity: usize,
}

impl StorageWriterHandle {
    pub fn try_enqueue(&self, command: StorageCommand) -> Result<(), StorageError> {
        match self.tx.try_send(command) {
            Ok(()) => {
                self.metrics.accepted.fetch_add(1, Ordering::Relaxed);
                Ok(())
            }
            Err(mpsc::error::TrySendError::Full(_)) => {
                self.metrics.dropped.fetch_add(1, Ordering::Relaxed);
                Err(StorageError::QueueFull)
            }
            Err(mpsc::error::TrySendError::Closed(_)) => Err(StorageError::WriterClosed),
        }
    }

    pub fn snapshot(&self) -> StorageWriterSnapshot {
        StorageWriterSnapshot {
            queued_capacity: self.capacity,
            queued_available: self.tx.capacity(),
            task_status: if self.tx.is_closed() {
                "stopped"
            } else {
                "running"
            },
            accepted: self.metrics.accepted.load(Ordering::Relaxed),
            dropped: self.metrics.dropped.load(Ordering::Relaxed),
            written: self.metrics.written.load(Ordering::Relaxed),
            failed: self.metrics.failed.load(Ordering::Relaxed),
            retried: self.metrics.retried.load(Ordering::Relaxed),
        }
    }
}

#[derive(Clone)]
pub struct StorageWriterRuntime {
    handle: StorageWriterHandle,
    _task: Arc<Mutex<Option<JoinHandle<()>>>>,
}

impl StorageWriterRuntime {
    pub fn new(handle: StorageWriterHandle, task: JoinHandle<()>) -> Self {
        Self {
            handle,
            _task: Arc::new(Mutex::new(Some(task))),
        }
    }

    pub fn handle(&self) -> &StorageWriterHandle {
        &self.handle
    }

    pub fn snapshot(&self) -> StorageWriterSnapshot {
        self.handle.snapshot()
    }
}

#[derive(Default)]
struct StorageWriterMetrics {
    accepted: AtomicU64,
    dropped: AtomicU64,
    written: AtomicU64,
    failed: AtomicU64,
    retried: AtomicU64,
}

pub struct StorageWriter;

impl StorageWriter {
    pub fn spawn<R>(
        repository: Arc<R>,
        capacity: usize,
        _flush_interval: Duration,
    ) -> (StorageWriterHandle, JoinHandle<()>)
    where
        R: StorageRepository + ?Sized + 'static,
    {
        let (tx, mut rx) = mpsc::channel::<StorageCommand>(capacity);
        let metrics = Arc::new(StorageWriterMetrics::default());
        let handle = StorageWriterHandle {
            tx,
            metrics: metrics.clone(),
            capacity,
        };

        let join = tokio::spawn(async move {
            while let Some(command) = rx.recv().await {
                write_one(repository.as_ref(), command, &metrics).await;

                while let Ok(command) = rx.try_recv() {
                    write_one(repository.as_ref(), command, &metrics).await;
                }
            }
        });

        (handle, join)
    }
}

async fn write_one<R>(repository: &R, command: StorageCommand, metrics: &StorageWriterMetrics)
where
    R: StorageRepository + ?Sized,
{
    let mut attempt = 1;
    let result = loop {
        let result = match &command {
            StorageCommand::RawMarketEvent(event) => {
                repository.insert_raw_market_event(event).await.map(|_| ())
            }
            StorageCommand::Tick(tick) => repository.insert_tick(tick).await.map(|_| ()),
            StorageCommand::Signal(signal) => repository.insert_signal(signal).await.map(|_| ()),
            StorageCommand::NotificationDelivery(delivery) => repository
                .insert_notification_delivery(delivery)
                .await
                .map(|_| ()),
            StorageCommand::RuntimeEvent(event) => {
                repository.insert_runtime_event(event).await.map(|_| ())
            }
        };

        if result.is_ok() || attempt >= MAX_WRITE_ATTEMPTS {
            break result;
        }

        metrics.retried.fetch_add(1, Ordering::Relaxed);
        attempt += 1;
        time::sleep(Duration::from_millis(25 * u64::from(attempt))).await;
    };

    match result {
        Ok(()) => {
            metrics.written.fetch_add(1, Ordering::Relaxed);
        }
        Err(error) => {
            metrics.failed.fetch_add(1, Ordering::Relaxed);
            tracing::warn!(%error, "storage writer command failed");
        }
    }
}
