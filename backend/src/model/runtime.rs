use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::{sync::mpsc, task::JoinHandle};

use crate::model::{ModelAction, ModelContext, ModelDecision, ModelRegistry};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ModelRuntimeError {
    #[error("model runtime queue is full")]
    QueueFull,
    #[error("model runtime queue is closed")]
    QueueClosed,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ModelRuntimeSnapshot {
    pub accepted: u64,
    pub dropped: u64,
    pub processed: u64,
    pub candidates: u64,
    pub no_trades: u64,
    pub failed: u64,
    pub last_action: Option<ModelAction>,
    pub last_decision: Option<ModelDecision>,
}

#[derive(Clone)]
pub struct ModelRuntime {
    tx: mpsc::Sender<ModelContext>,
    metrics: Arc<RwLock<ModelRuntimeSnapshot>>,
    _task: Arc<JoinHandle<()>>,
}

impl ModelRuntime {
    pub fn spawn(registry: ModelRegistry, capacity: usize) -> Self {
        let (tx, mut rx) = mpsc::channel::<ModelContext>(capacity);
        let metrics = Arc::new(RwLock::new(ModelRuntimeSnapshot::default()));
        let worker_metrics = metrics.clone();
        let task = tokio::spawn(async move {
            while let Some(context) = rx.recv().await {
                let decision = registry
                    .get(
                        &context.assignment.model_key,
                        &context.assignment.model_version,
                    )
                    .map(|model| model.decide(&context));

                let mut metrics = worker_metrics
                    .write()
                    .expect("model runtime metrics lock poisoned");
                metrics.processed += 1;
                match decision {
                    Some(decision) => {
                        metrics.last_action = Some(decision.action);
                        match decision.action {
                            ModelAction::Candidate => metrics.candidates += 1,
                            ModelAction::NoTrade => metrics.no_trades += 1,
                        }
                        metrics.last_decision = Some(decision);
                    }
                    None => {
                        metrics.failed += 1;
                        metrics.last_action = None;
                        metrics.last_decision = None;
                    }
                }
            }
        });

        Self {
            tx,
            metrics,
            _task: Arc::new(task),
        }
    }

    pub fn try_enqueue(&self, context: ModelContext) -> Result<(), ModelRuntimeError> {
        match self.tx.try_send(context) {
            Ok(()) => {
                let mut metrics = self
                    .metrics
                    .write()
                    .expect("model runtime metrics lock poisoned");
                metrics.accepted += 1;
                Ok(())
            }
            Err(mpsc::error::TrySendError::Full(_)) => {
                let mut metrics = self
                    .metrics
                    .write()
                    .expect("model runtime metrics lock poisoned");
                metrics.dropped += 1;
                Err(ModelRuntimeError::QueueFull)
            }
            Err(mpsc::error::TrySendError::Closed(_)) => Err(ModelRuntimeError::QueueClosed),
        }
    }

    pub fn snapshot(&self) -> ModelRuntimeSnapshot {
        self.metrics
            .read()
            .expect("model runtime metrics lock poisoned")
            .clone()
    }
}
