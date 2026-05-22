use std::{sync::Arc, sync::Mutex};

use tokio::{sync::mpsc, task::JoinHandle};

use crate::{
    notification::{NotificationJob, NotificationRuntime},
    storage::{SignalWithMarketRecord, StorageRepository},
};

#[derive(Clone)]
pub struct SignalNotificationBridge {
    _task: Arc<Mutex<Option<JoinHandle<()>>>>,
}

impl SignalNotificationBridge {
    pub fn spawn(
        mut rx: mpsc::Receiver<SignalWithMarketRecord>,
        storage: Arc<dyn StorageRepository>,
        notification: NotificationRuntime,
    ) -> Self {
        let task = tokio::spawn(async move {
            while let Some(signal) = rx.recv().await {
                if signal.signal_type != "actionable_alert" {
                    continue;
                }

                let channels = match storage.list_notification_channels(&signal.market_key).await {
                    Ok(channels) => channels,
                    Err(error) => {
                        tracing::warn!(
                            %error,
                            signal_id = %signal.id,
                            market_key = %signal.market_key,
                            "failed to load notification channels for signal"
                        );
                        continue;
                    }
                };

                if channels.is_empty() {
                    continue;
                }

                if let Err(error) = notification.try_enqueue(NotificationJob { signal, channels }) {
                    tracing::warn!(%error, "dropped signal notification job");
                }
            }
        });

        Self {
            _task: Arc::new(Mutex::new(Some(task))),
        }
    }
}
