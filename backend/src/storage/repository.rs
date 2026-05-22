use async_trait::async_trait;
use sqlx::{PgPool, Row};
use std::collections::BTreeMap;
use uuid::Uuid;

use crate::{
    realtime::MarketKey,
    storage::{
        BacktestRunRecord, CandleRecord, ModelAssignmentRecord, NewBacktestRun, NewModelAssignment,
        NewNotificationChannel, NewNotificationDelivery, NewRawMarketEvent, NewRuntimeEvent,
        NewSignal, NewTick, NotificationChannelRecord, RawMarketEventRecord, ReplayTick,
        RuntimeEventRecord, SignalRecord, SignalWithMarketRecord, StorageError, TickRecord,
    },
};

#[async_trait]
pub trait StorageRepository: Send + Sync {
    async fn insert_raw_market_event(
        &self,
        event: &NewRawMarketEvent,
    ) -> Result<RawMarketEventRecord, StorageError>;
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
    async fn recent_candles(
        &self,
        market_key: &str,
        limit: i64,
    ) -> Result<Vec<CandleRecord>, StorageError>;
    async fn latest_signals(
        &self,
        market_key: &str,
        limit: i64,
    ) -> Result<Vec<SignalWithMarketRecord>, StorageError>;
    async fn list_model_assignments(&self) -> Result<Vec<ModelAssignmentRecord>, StorageError>;
    async fn set_active_model_assignment(
        &self,
        assignment: &NewModelAssignment,
    ) -> Result<ModelAssignmentRecord, StorageError>;
    async fn list_notification_channels(
        &self,
        market_key: &str,
    ) -> Result<Vec<NotificationChannelRecord>, StorageError>;
    async fn upsert_notification_channel(
        &self,
        channel: &NewNotificationChannel,
    ) -> Result<NotificationChannelRecord, StorageError>;
    async fn insert_backtest_run(
        &self,
        run: &NewBacktestRun,
    ) -> Result<BacktestRunRecord, StorageError>;
    async fn latest_backtest_runs(
        &self,
        market_key: Option<&str>,
        model_key: Option<&str>,
        limit: i64,
    ) -> Result<Vec<BacktestRunRecord>, StorageError>;
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

    pub async fn load_realtime_market_ids(
        &self,
    ) -> Result<BTreeMap<MarketKey, Uuid>, StorageError> {
        let rows = sqlx::query(
            r#"
            SELECT market_key, id
            FROM markets
            WHERE market_key = ANY($1)
            "#,
        )
        .bind(&[MarketKey::Btc5m.as_str(), MarketKey::Eth15m.as_str()])
        .fetch_all(&self.pool)
        .await?;

        let mut market_ids = BTreeMap::new();
        for row in rows {
            let market_key: String = row.get("market_key");
            let market_id: Uuid = row.get("id");
            let market_key = market_key
                .parse::<MarketKey>()
                .map_err(|error| StorageError::InvalidInput(error.to_string()))?;
            market_ids.insert(market_key, market_id);
        }

        Ok(market_ids)
    }
}

#[async_trait]
impl StorageRepository for PostgresStorage {
    async fn insert_raw_market_event(
        &self,
        event: &NewRawMarketEvent,
    ) -> Result<RawMarketEventRecord, StorageError> {
        let record = if event.source_event_id.is_some() {
            sqlx::query_as::<_, RawMarketEventRecord>(
                r#"
                INSERT INTO raw_market_events (
                    source, source_event_id, received_at, source_ts, payload
                )
                VALUES ($1, $2, $3, $4, $5)
                ON CONFLICT (source, source_event_id) WHERE source_event_id IS NOT NULL
                DO UPDATE SET
                    received_at = EXCLUDED.received_at,
                    source_ts = EXCLUDED.source_ts,
                    payload = EXCLUDED.payload
                RETURNING id, source, source_event_id, received_at, source_ts, payload
                "#,
            )
            .bind(&event.source)
            .bind(&event.source_event_id)
            .bind(event.received_at)
            .bind(event.source_ts)
            .bind(&event.payload)
            .fetch_one(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, RawMarketEventRecord>(
                r#"
                INSERT INTO raw_market_events (
                    source, source_event_id, received_at, source_ts, payload
                )
                VALUES ($1, $2, $3, $4, $5)
                RETURNING id, source, source_event_id, received_at, source_ts, payload
                "#,
            )
            .bind(&event.source)
            .bind(&event.source_event_id)
            .bind(event.received_at)
            .bind(event.source_ts)
            .bind(&event.payload)
            .fetch_one(&self.pool)
            .await?
        };

        Ok(record)
    }

    async fn insert_tick(&self, tick: &NewTick) -> Result<TickRecord, StorageError> {
        let record = if tick.sequence.is_some() {
            sqlx::query_as::<_, TickRecord>(
                r#"
                INSERT INTO ticks (market_id, source, source_ts, received_at, price, size, sequence)
                VALUES ($1, $2, $3, $4, $5, $6, $7)
                ON CONFLICT (market_id, source, sequence) WHERE sequence IS NOT NULL
                DO UPDATE SET
                    source_ts = EXCLUDED.source_ts,
                    received_at = EXCLUDED.received_at,
                    price = EXCLUDED.price,
                    size = EXCLUDED.size
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
            .await?
        } else {
            sqlx::query_as::<_, TickRecord>(
                r#"
                INSERT INTO ticks (market_id, source, source_ts, received_at, price, size, sequence)
                VALUES ($1, $2, $3, $4, $5, $6, $7)
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
            .await?
        };

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

    async fn recent_candles(
        &self,
        market_key: &str,
        limit: i64,
    ) -> Result<Vec<CandleRecord>, StorageError> {
        let limit = limit.clamp(1, 500);
        let mut candles = sqlx::query_as::<_, CandleRecord>(
            r#"
            SELECT m.market_key, c.start_ts, c.open, c.high, c.low, c.close, c.volume
            FROM candles_1m c
            JOIN markets m ON m.id = c.market_id
            WHERE m.market_key = $1
            ORDER BY c.start_ts DESC
            LIMIT $2
            "#,
        )
        .bind(market_key)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        candles.reverse();
        Ok(candles)
    }

    async fn latest_signals(
        &self,
        market_key: &str,
        limit: i64,
    ) -> Result<Vec<SignalWithMarketRecord>, StorageError> {
        let limit = limit.clamp(1, 200);
        let signals = sqlx::query_as::<_, SignalWithMarketRecord>(
            r#"
            SELECT
                s.id,
                m.market_key,
                s.market_window_id,
                s.model_version_id,
                s.signal_type,
                s.side,
                s.confidence,
                s.limit_price,
                s.suggested_size,
                s.ttl_ms,
                s.reason,
                s.features,
                s.input_snapshot_hash,
                s.created_at
            FROM signals s
            JOIN market_windows w ON w.id = s.market_window_id
            JOIN markets m ON m.id = w.market_id
            WHERE m.market_key = $1
            ORDER BY s.created_at DESC
            LIMIT $2
            "#,
        )
        .bind(market_key)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(signals)
    }

    async fn list_model_assignments(&self) -> Result<Vec<ModelAssignmentRecord>, StorageError> {
        let assignments = sqlx::query_as::<_, ModelAssignmentRecord>(
            r#"
            SELECT
                m.market_key,
                mo.model_key,
                mo.display_name,
                mv.version,
                mv.parameters,
                ma.status,
                ma.created_at
            FROM model_assignments ma
            JOIN markets m ON m.id = ma.market_id
            JOIN model_versions mv ON mv.id = ma.model_version_id
            JOIN models mo ON mo.id = mv.model_id
            WHERE ma.status = 'active'
            ORDER BY m.market_key ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(assignments)
    }

    async fn set_active_model_assignment(
        &self,
        assignment: &NewModelAssignment,
    ) -> Result<ModelAssignmentRecord, StorageError> {
        let mut tx = self.pool.begin().await?;
        let market_id = sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT id FROM markets WHERE market_key = $1
            "#,
        )
        .bind(&assignment.market_key)
        .fetch_one(&mut *tx)
        .await?;

        let model_id = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO models (model_key, display_name)
            VALUES ($1, $2)
            ON CONFLICT (model_key)
            DO UPDATE SET display_name = EXCLUDED.display_name
            RETURNING id
            "#,
        )
        .bind(&assignment.model_key)
        .bind(&assignment.display_name)
        .fetch_one(&mut *tx)
        .await?;

        let model_version_id = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO model_versions (model_id, version, parameters)
            VALUES ($1, $2, $3)
            ON CONFLICT (model_id, version)
            DO UPDATE SET parameters = EXCLUDED.parameters
            RETURNING id
            "#,
        )
        .bind(model_id)
        .bind(&assignment.version)
        .bind(&assignment.parameters)
        .fetch_one(&mut *tx)
        .await?;

        sqlx::query(
            r#"
            UPDATE model_assignments
            SET status = 'inactive'
            WHERE market_id = $1
              AND status = 'active'
            "#,
        )
        .bind(market_id)
        .execute(&mut *tx)
        .await?;

        let record = sqlx::query_as::<_, ModelAssignmentRecord>(
            r#"
            WITH inserted AS (
                INSERT INTO model_assignments (market_id, model_version_id, status)
                VALUES ($1, $2, 'active')
                RETURNING market_id, model_version_id, status, created_at
            )
            SELECT
                m.market_key,
                mo.model_key,
                mo.display_name,
                mv.version,
                mv.parameters,
                inserted.status,
                inserted.created_at
            FROM inserted
            JOIN markets m ON m.id = inserted.market_id
            JOIN model_versions mv ON mv.id = inserted.model_version_id
            JOIN models mo ON mo.id = mv.model_id
            "#,
        )
        .bind(market_id)
        .bind(model_version_id)
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(record)
    }

    async fn list_notification_channels(
        &self,
        market_key: &str,
    ) -> Result<Vec<NotificationChannelRecord>, StorageError> {
        let channels = sqlx::query_as::<_, NotificationChannelRecord>(
            r#"
            SELECT
                nc.id,
                m.market_key,
                nc.channel_type,
                nc.name,
                nc.webhook_url,
                nc.enabled,
                nc.created_at
            FROM notification_channels nc
            JOIN markets m ON m.id = nc.market_id
            WHERE m.market_key = $1
            ORDER BY nc.name ASC
            "#,
        )
        .bind(market_key)
        .fetch_all(&self.pool)
        .await?;

        Ok(channels)
    }

    async fn upsert_notification_channel(
        &self,
        channel: &NewNotificationChannel,
    ) -> Result<NotificationChannelRecord, StorageError> {
        let record = sqlx::query_as::<_, NotificationChannelRecord>(
            r#"
            WITH target_market AS (
                SELECT id, market_key FROM markets WHERE market_key = $1
            ),
            upserted AS (
                INSERT INTO notification_channels (
                    market_id, channel_type, name, webhook_url, enabled
                )
                SELECT id, $2, $3, $4, $5
                FROM target_market
                ON CONFLICT (market_id, channel_type, name)
                DO UPDATE SET
                    webhook_url = EXCLUDED.webhook_url,
                    enabled = EXCLUDED.enabled
                RETURNING id, market_id, channel_type, name, webhook_url, enabled, created_at
            )
            SELECT
                upserted.id,
                target_market.market_key,
                upserted.channel_type,
                upserted.name,
                upserted.webhook_url,
                upserted.enabled,
                upserted.created_at
            FROM upserted
            JOIN target_market ON target_market.id = upserted.market_id
            "#,
        )
        .bind(&channel.market_key)
        .bind(&channel.channel_type)
        .bind(&channel.name)
        .bind(&channel.webhook_url)
        .bind(channel.enabled)
        .fetch_one(&self.pool)
        .await?;

        Ok(record)
    }

    async fn insert_backtest_run(
        &self,
        run: &NewBacktestRun,
    ) -> Result<BacktestRunRecord, StorageError> {
        let mut tx = self.pool.begin().await?;
        let market_id = sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT id FROM markets WHERE market_key = $1
            "#,
        )
        .bind(&run.market_key)
        .fetch_one(&mut *tx)
        .await?;

        let model_id = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO models (model_key, display_name)
            VALUES ($1, $2)
            ON CONFLICT (model_key)
            DO UPDATE SET display_name = EXCLUDED.display_name
            RETURNING id
            "#,
        )
        .bind(&run.model_key)
        .bind(&run.display_name)
        .fetch_one(&mut *tx)
        .await?;

        let model_version_id = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO model_versions (model_id, version, parameters)
            VALUES ($1, $2, $3)
            ON CONFLICT (model_id, version)
            DO UPDATE SET parameters = EXCLUDED.parameters
            RETURNING id
            "#,
        )
        .bind(model_id)
        .bind(&run.model_version)
        .bind(&run.parameters)
        .fetch_one(&mut *tx)
        .await?;

        let record = sqlx::query_as::<_, BacktestRunRecord>(
            r#"
            WITH inserted AS (
                INSERT INTO backtest_runs (
                    model_version_id, market_id, finished_at, window_start,
                    window_end, metrics, status
                )
                VALUES (
                    $1,
                    $2,
                    CASE WHEN $6 = 'completed' THEN now() ELSE NULL END,
                    $3,
                    $4,
                    $5,
                    $6
                )
                RETURNING id, model_version_id, market_id, started_at, finished_at,
                    window_start, window_end, metrics, status
            )
            SELECT
                inserted.id,
                m.market_key,
                mo.model_key,
                mo.display_name,
                mv.version AS model_version,
                mv.parameters,
                inserted.started_at,
                inserted.finished_at,
                inserted.window_start,
                inserted.window_end,
                inserted.metrics,
                inserted.status
            FROM inserted
            JOIN markets m ON m.id = inserted.market_id
            JOIN model_versions mv ON mv.id = inserted.model_version_id
            JOIN models mo ON mo.id = mv.model_id
            "#,
        )
        .bind(model_version_id)
        .bind(market_id)
        .bind(run.window_start)
        .bind(run.window_end)
        .bind(&run.metrics)
        .bind(&run.status)
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(record)
    }

    async fn latest_backtest_runs(
        &self,
        market_key: Option<&str>,
        model_key: Option<&str>,
        limit: i64,
    ) -> Result<Vec<BacktestRunRecord>, StorageError> {
        let limit = limit.clamp(1, 100);
        let runs = sqlx::query_as::<_, BacktestRunRecord>(
            r#"
            SELECT
                br.id,
                m.market_key,
                mo.model_key,
                mo.display_name,
                mv.version AS model_version,
                mv.parameters,
                br.started_at,
                br.finished_at,
                br.window_start,
                br.window_end,
                br.metrics,
                br.status
            FROM backtest_runs br
            JOIN markets m ON m.id = br.market_id
            JOIN model_versions mv ON mv.id = br.model_version_id
            JOIN models mo ON mo.id = mv.model_id
            WHERE ($1::text IS NULL OR m.market_key = $1)
              AND ($2::text IS NULL OR mo.model_key = $2)
            ORDER BY br.started_at DESC
            LIMIT $3
            "#,
        )
        .bind(market_key)
        .bind(model_key)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(runs)
    }
}
