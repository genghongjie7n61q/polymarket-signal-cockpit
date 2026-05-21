use async_trait::async_trait;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::storage::{
    NewNotificationDelivery, NewRuntimeEvent, NewSignal, NewTick, ReplayTick, RuntimeEventRecord,
    SignalRecord, StorageError, TickRecord,
};

#[async_trait]
pub trait StorageRepository: Send + Sync {
    async fn insert_tick(&self, tick: &NewTick) -> Result<TickRecord, StorageError>;
    async fn insert_signal(&self, signal: &NewSignal) -> Result<SignalRecord, StorageError>;
    async fn insert_notification_delivery(
        &self,
        delivery: &NewNotificationDelivery,
    ) -> Result<Uuid, StorageError>;
    async fn insert_runtime_event(
        &self,
        event: &NewRuntimeEvent,
    ) -> Result<RuntimeEventRecord, StorageError>;
    async fn replay_ticks_for_window(
        &self,
        market_key: &str,
        window_start: time::OffsetDateTime,
    ) -> Result<Vec<ReplayTick>, StorageError>;
}

#[derive(Clone)]
pub struct PostgresStorage {
    pool: PgPool,
}

impl PostgresStorage {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

#[async_trait]
impl StorageRepository for PostgresStorage {
    async fn insert_tick(&self, tick: &NewTick) -> Result<TickRecord, StorageError> {
        let record = sqlx::query_as::<_, TickRecord>(
            r#"
            INSERT INTO ticks (market_id, source, source_ts, received_at, price, size, sequence)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (market_id, source, source_ts, price, (COALESCE(sequence, -1)))
            DO UPDATE SET received_at = EXCLUDED.received_at
            RETURNING id, market_id, source, source_ts, received_at, price, size, sequence
            "#,
        )
        .bind(tick.market_id)
        .bind(&tick.source)
        .bind(tick.source_ts)
        .bind(tick.received_at)
        .bind(&tick.price)
        .bind(&tick.size)
        .bind(tick.sequence)
        .fetch_one(&self.pool)
        .await?;

        Ok(record)
    }

    async fn insert_signal(&self, signal: &NewSignal) -> Result<SignalRecord, StorageError> {
        let record = sqlx::query_as::<_, SignalRecord>(
            r#"
            INSERT INTO signals (
                market_window_id, model_version_id, signal_type, side, confidence,
                limit_price, suggested_size, ttl_ms, reason, features, input_snapshot_hash
            )
            VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)
            ON CONFLICT (
                market_window_id,
                model_version_id,
                signal_type,
                (COALESCE(side, '')),
                input_snapshot_hash
            )
            DO UPDATE SET reason = EXCLUDED.reason
            RETURNING id, market_window_id, model_version_id, signal_type, side, confidence,
                limit_price, suggested_size, ttl_ms, reason, features, input_snapshot_hash, created_at
            "#,
        )
        .bind(signal.market_window_id)
        .bind(signal.model_version_id)
        .bind(&signal.signal_type)
        .bind(&signal.side)
        .bind(&signal.confidence)
        .bind(&signal.limit_price)
        .bind(&signal.suggested_size)
        .bind(signal.ttl_ms)
        .bind(&signal.reason)
        .bind(&signal.features)
        .bind(&signal.input_snapshot_hash)
        .fetch_one(&self.pool)
        .await?;

        Ok(record)
    }

    async fn insert_notification_delivery(
        &self,
        delivery: &NewNotificationDelivery,
    ) -> Result<Uuid, StorageError> {
        let id = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO notification_deliveries (
                signal_id, channel_id, dedupe_key, status, attempt_count, response_summary
            )
            VALUES ($1,$2,$3,$4,$5,$6)
            ON CONFLICT (dedupe_key, channel_id)
            DO UPDATE SET
                status = EXCLUDED.status,
                attempt_count = EXCLUDED.attempt_count,
                response_summary = EXCLUDED.response_summary,
                updated_at = now()
            RETURNING id
            "#,
        )
        .bind(delivery.signal_id)
        .bind(delivery.channel_id)
        .bind(&delivery.dedupe_key)
        .bind(&delivery.status)
        .bind(delivery.attempt_count)
        .bind(&delivery.response_summary)
        .fetch_one(&self.pool)
        .await?;

        Ok(id)
    }

    async fn insert_runtime_event(
        &self,
        event: &NewRuntimeEvent,
    ) -> Result<RuntimeEventRecord, StorageError> {
        let record = sqlx::query_as::<_, RuntimeEventRecord>(
            r#"
            INSERT INTO runtime_events (component, severity, event_type, message, details)
            VALUES ($1,$2,$3,$4,$5)
            RETURNING id, component, severity, event_type, message, details, created_at
            "#,
        )
        .bind(&event.component)
        .bind(&event.severity)
        .bind(&event.event_type)
        .bind(&event.message)
        .bind(&event.details)
        .fetch_one(&self.pool)
        .await?;

        Ok(record)
    }

    async fn replay_ticks_for_window(
        &self,
        market_key: &str,
        window_start: time::OffsetDateTime,
    ) -> Result<Vec<ReplayTick>, StorageError> {
        let rows = sqlx::query(
            r#"
            SELECT m.market_key, w.start_ts, w.end_ts, t.source_ts, t.price, t.size
            FROM ticks t
            JOIN markets m ON m.id = t.market_id
            JOIN market_windows w ON w.market_id = m.id
            WHERE m.market_key = $1
              AND w.start_ts = $2
              AND t.source_ts >= w.start_ts
              AND t.source_ts < w.end_ts
            ORDER BY t.source_ts ASC
            "#,
        )
        .bind(market_key)
        .bind(window_start)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| ReplayTick {
                market_key: row.get("market_key"),
                window_start: row.get("start_ts"),
                window_end: row.get("end_ts"),
                source_ts: row.get("source_ts"),
                price: row.get("price"),
                size: row.get("size"),
            })
            .collect())
    }
}
