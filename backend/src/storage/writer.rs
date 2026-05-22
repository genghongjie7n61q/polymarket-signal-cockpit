use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

use tokio::{sync::mpsc, task::JoinHandle, time};

use crate::storage::{
    NewNotificationDelivery, NewRawMarketEvent, NewRuntimeEvent, NewSignal, NewTick,
    SignalWithMarketRecord, StorageError, StorageRepository,
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
        Self::spawn_internal(repository, capacity, None)
    }

    pub fn spawn_with_signal_notifier<R>(
        repository: Arc<R>,
        capacity: usize,
        _flush_interval: Duration,
        signal_tx: mpsc::Sender<SignalWithMarketRecord>,
    ) -> (StorageWriterHandle, JoinHandle<()>)
    where
        R: StorageRepository + ?Sized + 'static,
    {
        Self::spawn_internal(repository, capacity, Some(signal_tx))
    }

    fn spawn_internal<R>(
        repository: Arc<R>,
        capacity: usize,
        signal_tx: Option<mpsc::Sender<SignalWithMarketRecord>>,
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
                write_one(repository.as_ref(), command, &metrics, signal_tx.as_ref()).await;

                while let Ok(command) = rx.try_recv() {
                    write_one(repository.as_ref(), command, &metrics, signal_tx.as_ref()).await;
                }
            }
        });

        (handle, join)
    }
}

async fn write_one<R>(
    repository: &R,
    command: StorageCommand,
    metrics: &StorageWriterMetrics,
    signal_tx: Option<&mpsc::Sender<SignalWithMarketRecord>>,
) where
    R: StorageRepository + ?Sized,
{
    let mut attempt = 1;
    let result = loop {
        let result = match &command {
            StorageCommand::RawMarketEvent(event) => {
                repository.insert_raw_market_event(event).await.map(|_| ())
            }
            StorageCommand::Tick(tick) => repository.insert_tick(tick).await.map(|_| ()),
            StorageCommand::Signal(signal) => match repository.insert_signal(signal).await {
                Ok(record) => {
                    notify_inserted_signal(repository, record.id, signal_tx).await;
                    Ok(())
                }
                Err(error) => Err(error),
            },
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

async fn notify_inserted_signal<R>(
    repository: &R,
    signal_id: uuid::Uuid,
    signal_tx: Option<&mpsc::Sender<SignalWithMarketRecord>>,
) where
    R: StorageRepository + ?Sized,
{
    let Some(signal_tx) = signal_tx else {
        return;
    };
    let signal = match repository.signal_with_market(signal_id).await {
        Ok(signal) => signal,
        Err(error) => {
            tracing::warn!(%error, %signal_id, "failed to load inserted signal for notification");
            return;
        }
    };
    if let Err(error) = signal_tx.try_send(signal) {
        tracing::warn!(%error, %signal_id, "dropped inserted signal notification handoff");
    }
}
