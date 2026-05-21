# WEB-6 M1 数据与存储底座 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the PostgreSQL schema, migrations, typed storage APIs, bounded async storage writer, runtime events, and replay queries required by `WEB-6 / M1 数据与存储底座`.

**Architecture:** The backend will use `sqlx` with PostgreSQL migrations embedded in the Rust crate. Domain records live in small storage modules, synchronous callers enqueue `StorageCommand` values into a bounded Tokio channel, and a background writer batches database writes so realtime evaluation is not blocked by database I/O.

**Tech Stack:** Rust, Tokio, Axum, sqlx, PostgreSQL 16 on dev-2 via podman-compose, serde/serde_json, uuid, time, bigdecimal.

---

## Non-Negotiable Project Rules

- Do not install or run local Mac runtime services.
- Do not run PostgreSQL containers on the local Mac.
- Unit and integration tests are executed on `dev-2`.
- Container/runtime validation is executed on `dev-2` and must use `192.168.103.157` for service health checks.
- Do not commit secrets or `.env`.
- Do not add real Polymarket trading, private key handling, or geo-bypass behavior.

## File Structure

Create:

- `backend/migrations/20260521000100_initial_storage.sql` - initial PostgreSQL schema and indexes.
- `backend/src/storage/mod.rs` - storage module exports and public types.
- `backend/src/storage/error.rs` - storage error type.
- `backend/src/storage/types.rs` - typed domain records and enums used by repositories and writer.
- `backend/src/storage/pool.rs` - database pool creation and migration runner.
- `backend/src/storage/repository.rs` - typed insert/read API for ticks, signals, notifications, runtime events, and replay queries.
- `backend/src/storage/writer.rs` - bounded channel writer and batch flushing.
- `backend/tests/storage_migrations_tests.rs` - migration and schema tests.
- `backend/tests/storage_repository_tests.rs` - repository insert/read tests.
- `backend/tests/storage_writer_tests.rs` - bounded writer and DB outage tests.
- `backend/tests/dev2_storage_validation.rs` - ignored dev-2 integration smoke covering real Postgres.
- `docs/dev-2-web-6-validation.md` - exact dev-2 validation and cleanup runbook.

Modify:

- `backend/Cargo.toml` - add `sqlx`, `uuid`, `time`, `bigdecimal`, `async-trait`, and Tokio sync/time features.
- `backend/src/lib.rs` - export `storage`.
- `backend/src/router.rs` - carry optional storage handles in `AppState` only if needed for health/runtime status.
- `backend/src/health.rs` - include basic storage configuration/queue status only after the storage module exposes a cheap snapshot.
- `docker-compose.yml` - keep PostgreSQL service; no local-only assumptions.
- `.env.example` - document storage writer queue/batch variables.
- `README.md` - document WEB-6 dev-2 test/validation commands.

## Task 1: Dependencies And Storage Module Skeleton

**Files:**

- Modify: `backend/Cargo.toml`
- Modify: `backend/src/lib.rs`
- Create: `backend/src/storage/mod.rs`
- Create: `backend/src/storage/error.rs`

- [ ] **Step 1: Add backend dependencies**

Update `backend/Cargo.toml`:

```toml
[dependencies]
axum = "0.8"
async-trait = "0.1"
bigdecimal = { version = "0.4", features = ["serde"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
sqlx = { version = "0.8", features = ["runtime-tokio-rustls", "postgres", "uuid", "time", "json", "bigdecimal", "macros", "migrate"] }
thiserror = "1"
time = { version = "0.3", features = ["serde", "formatting", "parsing"] }
tokio = { version = "1", features = ["macros", "net", "rt-multi-thread", "signal", "sync", "time"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "fmt"] }
uuid = { version = "1", features = ["v4", "serde"] }
```

- [ ] **Step 2: Create storage module exports**

Create `backend/src/storage/mod.rs`:

```rust
pub mod error;
pub mod pool;
pub mod repository;
pub mod types;
pub mod writer;

pub use error::StorageError;
pub use pool::{connect_pool, run_migrations, PgPoolOptionsConfig};
pub use repository::{PostgresStorage, StorageRepository};
pub use types::*;
pub use writer::{StorageCommand, StorageWriter, StorageWriterHandle, StorageWriterSnapshot};
```

- [ ] **Step 3: Create storage error type**

Create `backend/src/storage/error.rs`:

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("storage queue is full")]
    QueueFull,
    #[error("storage writer is closed")]
    WriterClosed,
    #[error("invalid storage input: {0}")]
    InvalidInput(String),
}
```

- [ ] **Step 4: Export storage from lib**

Modify `backend/src/lib.rs`:

```rust
pub mod config;
pub mod health;
pub mod router;
pub mod storage;
```

- [ ] **Step 5: Commit Task 1**

Run:

```bash
git add backend/Cargo.toml backend/src/lib.rs backend/src/storage/mod.rs backend/src/storage/error.rs
git commit -m "feat: add storage module skeleton"
```

Expected: commit succeeds on branch `codex/web-6-storage-foundation`.

## Task 2: Initial PostgreSQL Migration

**Files:**

- Create: `backend/migrations/20260521000100_initial_storage.sql`
- Create: `backend/tests/storage_migrations_tests.rs`
- Create: `backend/src/storage/pool.rs`

- [ ] **Step 1: Write migration file**

Create `backend/migrations/20260521000100_initial_storage.sql`:

```sql
CREATE EXTENSION IF NOT EXISTS pgcrypto;

CREATE TABLE IF NOT EXISTS assets (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    symbol TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS markets (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    market_key TEXT NOT NULL UNIQUE,
    asset_id UUID NOT NULL REFERENCES assets(id),
    symbol TEXT NOT NULL,
    interval_seconds INTEGER NOT NULL CHECK (interval_seconds > 0),
    source TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS market_windows (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    market_id UUID NOT NULL REFERENCES markets(id),
    event_slug TEXT,
    start_ts TIMESTAMPTZ NOT NULL,
    end_ts TIMESTAMPTZ NOT NULL,
    status TEXT NOT NULL DEFAULT 'open',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (market_id, start_ts),
    CHECK (end_ts > start_ts)
);

CREATE TABLE IF NOT EXISTS raw_market_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    source TEXT NOT NULL,
    source_event_id TEXT,
    received_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    source_ts TIMESTAMPTZ,
    payload JSONB NOT NULL,
    UNIQUE (source, source_event_id)
);

CREATE TABLE IF NOT EXISTS ticks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    market_id UUID NOT NULL REFERENCES markets(id),
    raw_event_id UUID REFERENCES raw_market_events(id),
    source TEXT NOT NULL,
    source_ts TIMESTAMPTZ NOT NULL,
    received_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    price NUMERIC(28, 10) NOT NULL,
    size NUMERIC(28, 10),
    sequence BIGINT
);

CREATE TABLE IF NOT EXISTS candles_1m (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    market_id UUID NOT NULL REFERENCES markets(id),
    start_ts TIMESTAMPTZ NOT NULL,
    open NUMERIC(28, 10) NOT NULL,
    high NUMERIC(28, 10) NOT NULL,
    low NUMERIC(28, 10) NOT NULL,
    close NUMERIC(28, 10) NOT NULL,
    volume NUMERIC(28, 10) NOT NULL DEFAULT 0,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (market_id, start_ts)
);

CREATE TABLE IF NOT EXISTS polymarket_snapshots (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    market_window_id UUID NOT NULL REFERENCES market_windows(id),
    captured_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    event_slug TEXT NOT NULL,
    up_price NUMERIC(10, 6),
    down_price NUMERIC(10, 6),
    spread NUMERIC(10, 6),
    liquidity NUMERIC(28, 10),
    payload JSONB NOT NULL,
    UNIQUE (market_window_id, captured_at)
);

CREATE TABLE IF NOT EXISTS models (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    model_key TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS model_versions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    model_id UUID NOT NULL REFERENCES models(id),
    version TEXT NOT NULL,
    parameters JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (model_id, version)
);

CREATE TABLE IF NOT EXISTS model_assignments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    market_id UUID NOT NULL REFERENCES markets(id),
    model_version_id UUID NOT NULL REFERENCES model_versions(id),
    status TEXT NOT NULL DEFAULT 'active',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (market_id, status)
);

CREATE TABLE IF NOT EXISTS backtest_runs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    model_version_id UUID NOT NULL REFERENCES model_versions(id),
    market_id UUID NOT NULL REFERENCES markets(id),
    started_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    finished_at TIMESTAMPTZ,
    window_start TIMESTAMPTZ NOT NULL,
    window_end TIMESTAMPTZ NOT NULL,
    metrics JSONB NOT NULL DEFAULT '{}'::jsonb,
    status TEXT NOT NULL DEFAULT 'running'
);

CREATE TABLE IF NOT EXISTS signals (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    market_window_id UUID NOT NULL REFERENCES market_windows(id),
    model_version_id UUID NOT NULL REFERENCES model_versions(id),
    signal_type TEXT NOT NULL,
    side TEXT,
    confidence NUMERIC(10, 6),
    limit_price NUMERIC(10, 6),
    suggested_size NUMERIC(28, 10),
    ttl_ms INTEGER,
    reason TEXT NOT NULL,
    features JSONB NOT NULL DEFAULT '{}'::jsonb,
    input_snapshot_hash TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS paper_orders (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    signal_id UUID NOT NULL REFERENCES signals(id),
    status TEXT NOT NULL DEFAULT 'suggested',
    side TEXT NOT NULL,
    limit_price NUMERIC(10, 6) NOT NULL,
    suggested_size NUMERIC(28, 10) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS notification_channels (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    market_id UUID NOT NULL REFERENCES markets(id),
    channel_type TEXT NOT NULL,
    name TEXT NOT NULL,
    webhook_url TEXT NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (market_id, channel_type, name)
);

CREATE TABLE IF NOT EXISTS notification_deliveries (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    signal_id UUID NOT NULL REFERENCES signals(id),
    channel_id UUID NOT NULL REFERENCES notification_channels(id),
    dedupe_key TEXT NOT NULL,
    status TEXT NOT NULL,
    attempt_count INTEGER NOT NULL DEFAULT 0,
    response_summary TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (dedupe_key, channel_id)
);

CREATE TABLE IF NOT EXISTS runtime_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    component TEXT NOT NULL,
    severity TEXT NOT NULL,
    event_type TEXT NOT NULL,
    message TEXT NOT NULL,
    details JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_ticks_market_source_ts ON ticks (market_id, source_ts DESC);
CREATE UNIQUE INDEX IF NOT EXISTS idx_ticks_idempotency
    ON ticks (market_id, source, source_ts, price, COALESCE(sequence, -1));
CREATE INDEX IF NOT EXISTS idx_market_windows_market_start ON market_windows (market_id, start_ts DESC);
CREATE INDEX IF NOT EXISTS idx_signals_window_created ON signals (market_window_id, created_at);
CREATE UNIQUE INDEX IF NOT EXISTS idx_signals_idempotency
    ON signals (
        market_window_id,
        model_version_id,
        signal_type,
        COALESCE(side, ''),
        input_snapshot_hash
    );
CREATE INDEX IF NOT EXISTS idx_runtime_events_created ON runtime_events (created_at DESC);
```

- [ ] **Step 2: Add migration runner**

Create `backend/src/storage/pool.rs`:

```rust
use sqlx::{postgres::PgPoolOptions, PgPool};

#[derive(Debug, Clone, Copy)]
pub struct PgPoolOptionsConfig {
    pub max_connections: u32,
}

impl Default for PgPoolOptionsConfig {
    fn default() -> Self {
        Self { max_connections: 5 }
    }
}

pub async fn connect_pool(
    database_url: &str,
    options: PgPoolOptionsConfig,
) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(options.max_connections)
        .connect(database_url)
        .await
}

pub async fn run_migrations(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("./migrations").run(pool).await
}
```

- [ ] **Step 3: Add migration smoke test**

Create `backend/tests/storage_migrations_tests.rs`:

```rust
use polymarket_backend::storage::{connect_pool, run_migrations, PgPoolOptionsConfig};

fn database_url() -> Option<String> {
    std::env::var("DATABASE_URL").ok().filter(|value| !value.is_empty())
}

#[tokio::test]
async fn migrations_apply_and_core_tables_exist() {
    let Some(database_url) = database_url() else {
        eprintln!("DATABASE_URL not set; skipping PostgreSQL migration test");
        return;
    };

    let pool = connect_pool(
        &database_url,
        PgPoolOptionsConfig { max_connections: 2 },
    )
    .await
    .expect("test database should connect");

    run_migrations(&pool)
        .await
        .expect("migrations should apply");
    run_migrations(&pool)
        .await
        .expect("migrations should be repeatable");

    let table_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM information_schema.tables WHERE table_schema = 'public' AND table_name = ANY($1)",
    )
    .bind(&vec![
        "assets",
        "markets",
        "market_windows",
        "ticks",
        "signals",
        "notification_deliveries",
        "runtime_events",
    ])
    .fetch_one(&pool)
    .await
    .expect("table count query should work");

    assert_eq!(table_count, 7);
}
```

- [ ] **Step 4: Run test on dev-2**

Run from local after pushing this task branch:

```bash
ssh dev-2 'cd /opt/polymarket-signal-cockpit && git fetch origin codex/web-6-storage-foundation && git checkout codex/web-6-storage-foundation'
ssh dev-2 'cd /opt/polymarket-signal-cockpit && podman-compose up -d postgres'
ssh dev-2 'cd /opt/polymarket-signal-cockpit && DATABASE_URL=postgres://polymarket:dev2-local-polymarket-password@127.0.0.1:5432/polymarket cargo test -p polymarket-backend storage_migrations_tests -- --nocapture'
```

Expected: migration test passes on dev-2.

- [ ] **Step 5: Commit Task 2**

Run:

```bash
git add backend/migrations/20260521000100_initial_storage.sql backend/src/storage/pool.rs backend/tests/storage_migrations_tests.rs
git commit -m "feat: add initial storage migrations"
```

## Task 3: Domain Types

**Files:**

- Create: `backend/src/storage/types.rs`
- Test: `backend/tests/storage_repository_tests.rs`

- [ ] **Step 1: Define storage domain types**

Create `backend/src/storage/types.rs`:

```rust
use bigdecimal::BigDecimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::FromRow;
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, FromRow)]
pub struct AssetRecord {
    pub id: Uuid,
    pub symbol: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, FromRow)]
pub struct MarketRecord {
    pub id: Uuid,
    pub market_key: String,
    pub asset_id: Uuid,
    pub symbol: String,
    pub interval_seconds: i32,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NewTick {
    pub market_id: Uuid,
    pub source: String,
    pub source_ts: OffsetDateTime,
    pub received_at: OffsetDateTime,
    pub price: BigDecimal,
    pub size: Option<BigDecimal>,
    pub sequence: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, FromRow)]
pub struct TickRecord {
    pub id: Uuid,
    pub market_id: Uuid,
    pub source: String,
    pub source_ts: OffsetDateTime,
    pub received_at: OffsetDateTime,
    pub price: BigDecimal,
    pub size: Option<BigDecimal>,
    pub sequence: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NewSignal {
    pub market_window_id: Uuid,
    pub model_version_id: Uuid,
    pub signal_type: String,
    pub side: Option<String>,
    pub confidence: Option<BigDecimal>,
    pub limit_price: Option<BigDecimal>,
    pub suggested_size: Option<BigDecimal>,
    pub ttl_ms: Option<i32>,
    pub reason: String,
    pub features: Value,
    pub input_snapshot_hash: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, FromRow)]
pub struct SignalRecord {
    pub id: Uuid,
    pub market_window_id: Uuid,
    pub model_version_id: Uuid,
    pub signal_type: String,
    pub side: Option<String>,
    pub confidence: Option<BigDecimal>,
    pub limit_price: Option<BigDecimal>,
    pub suggested_size: Option<BigDecimal>,
    pub ttl_ms: Option<i32>,
    pub reason: String,
    pub features: Value,
    pub input_snapshot_hash: String,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewRuntimeEvent {
    pub component: String,
    pub severity: String,
    pub event_type: String,
    pub message: String,
    pub details: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, FromRow)]
pub struct RuntimeEventRecord {
    pub id: Uuid,
    pub component: String,
    pub severity: String,
    pub event_type: String,
    pub message: String,
    pub details: Value,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NewNotificationDelivery {
    pub signal_id: Uuid,
    pub channel_id: Uuid,
    pub dedupe_key: String,
    pub status: String,
    pub attempt_count: i32,
    pub response_summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReplayTick {
    pub market_key: String,
    pub window_start: OffsetDateTime,
    pub window_end: OffsetDateTime,
    pub source_ts: OffsetDateTime,
    pub price: BigDecimal,
    pub size: Option<BigDecimal>,
}
```

- [ ] **Step 2: Add compile smoke test**

Create or extend `backend/tests/storage_repository_tests.rs`:

```rust
use bigdecimal::BigDecimal;
use polymarket_backend::storage::NewTick;
use time::OffsetDateTime;
use uuid::Uuid;

#[test]
fn new_tick_type_is_serializable() {
    let tick = NewTick {
        market_id: Uuid::nil(),
        source: "coinbase".to_string(),
        source_ts: OffsetDateTime::UNIX_EPOCH,
        received_at: OffsetDateTime::UNIX_EPOCH,
        price: BigDecimal::from(100),
        size: Some(BigDecimal::from(2)),
        sequence: Some(1),
    };

    let encoded = serde_json::to_value(&tick).expect("tick should serialize");
    assert_eq!(encoded["source"], "coinbase");
}
```

- [ ] **Step 3: Commit Task 3**

Run:

```bash
git add backend/src/storage/types.rs backend/tests/storage_repository_tests.rs
git commit -m "feat: add storage domain types"
```

## Task 4: Typed Repository API

**Files:**

- Create: `backend/src/storage/repository.rs`
- Modify: `backend/tests/storage_repository_tests.rs`

- [ ] **Step 1: Create repository trait and Postgres implementation**

Create `backend/src/storage/repository.rs`:

```rust
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
            "#
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
            "#
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
            "#
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
            "#
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
```

- [ ] **Step 2: Add repository integration tests**

Extend `backend/tests/storage_repository_tests.rs` with database-backed tests guarded by `DATABASE_URL`. Include helper setup SQL for asset, market, window, model, model_version, and channel records.

Required test names:

```rust
#[tokio::test]
async fn repository_inserts_tick_idempotently() { /* ... */ }

#[tokio::test]
async fn repository_inserts_signal_idempotently() { /* ... */ }

#[tokio::test]
async fn repository_inserts_notification_delivery_idempotently() { /* ... */ }

#[tokio::test]
async fn repository_queries_replay_ticks_by_market_window() { /* ... */ }
```

Each test must skip with a clear message when `DATABASE_URL` is not set, then pass on dev-2.

- [ ] **Step 3: Run repository tests on dev-2**

Run:

```bash
ssh dev-2 'cd /opt/polymarket-signal-cockpit && git fetch origin codex/web-6-storage-foundation && git checkout codex/web-6-storage-foundation'
ssh dev-2 'cd /opt/polymarket-signal-cockpit && DATABASE_URL=postgres://polymarket:dev2-local-polymarket-password@127.0.0.1:5432/polymarket cargo test -p polymarket-backend storage_repository_tests -- --nocapture'
```

Expected: repository tests pass on dev-2.

- [ ] **Step 4: Commit Task 4**

Run:

```bash
git add backend/src/storage/repository.rs backend/tests/storage_repository_tests.rs
git commit -m "feat: add typed storage repository"
```

## Task 5: Bounded Async Storage Writer

**Files:**

- Create: `backend/src/storage/writer.rs`
- Create: `backend/tests/storage_writer_tests.rs`

- [ ] **Step 1: Implement bounded writer types**

Create `backend/src/storage/writer.rs`:

```rust
use std::sync::{
    atomic::{AtomicU64, AtomicUsize, Ordering},
    Arc,
};

use tokio::{
    sync::mpsc,
    task::JoinHandle,
    time::{self, Duration},
};

use crate::storage::{
    NewNotificationDelivery, NewRuntimeEvent, NewSignal, NewTick, StorageError,
    StorageRepository,
};

#[derive(Debug, Clone)]
pub enum StorageCommand {
    Tick(NewTick),
    Signal(NewSignal),
    NotificationDelivery(NewNotificationDelivery),
    RuntimeEvent(NewRuntimeEvent),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageWriterSnapshot {
    pub queued_capacity: usize,
    pub queued_available: usize,
    pub accepted: u64,
    pub dropped: u64,
    pub written: u64,
    pub failed: u64,
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
            accepted: self.metrics.accepted.load(Ordering::Relaxed),
            dropped: self.metrics.dropped.load(Ordering::Relaxed),
            written: self.metrics.written.load(Ordering::Relaxed),
            failed: self.metrics.failed.load(Ordering::Relaxed),
        }
    }
}

#[derive(Default)]
struct StorageWriterMetrics {
    accepted: AtomicU64,
    dropped: AtomicU64,
    written: AtomicU64,
    failed: AtomicU64,
}

pub struct StorageWriter;

impl StorageWriter {
    pub fn spawn<R>(
        repository: Arc<R>,
        capacity: usize,
        flush_interval: Duration,
    ) -> (StorageWriterHandle, JoinHandle<()>)
    where
        R: StorageRepository + 'static,
    {
        let (tx, mut rx) = mpsc::channel::<StorageCommand>(capacity);
        let metrics = Arc::new(StorageWriterMetrics::default());
        let handle = StorageWriterHandle {
            tx,
            metrics: metrics.clone(),
            capacity,
        };

        let join = tokio::spawn(async move {
            let mut interval = time::interval(flush_interval);
            loop {
                tokio::select! {
                    Some(command) = rx.recv() => {
                        write_one(repository.as_ref(), command, &metrics).await;
                    }
                    _ = interval.tick() => {}
                    else => break,
                }
            }
        });

        (handle, join)
    }
}

async fn write_one<R>(repository: &R, command: StorageCommand, metrics: &StorageWriterMetrics)
where
    R: StorageRepository,
{
    let result = match command {
        StorageCommand::Tick(tick) => repository.insert_tick(&tick).await.map(|_| ()),
        StorageCommand::Signal(signal) => repository.insert_signal(&signal).await.map(|_| ()),
        StorageCommand::NotificationDelivery(delivery) => repository
            .insert_notification_delivery(&delivery)
            .await
            .map(|_| ()),
        StorageCommand::RuntimeEvent(event) => repository.insert_runtime_event(&event).await.map(|_| ()),
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
```

- [ ] **Step 2: Add writer tests with fake repository**

Create `backend/tests/storage_writer_tests.rs` with an in-memory fake repository implementing `StorageRepository`.

Required test names:

```rust
#[tokio::test]
async fn storage_writer_try_enqueue_does_not_block_when_capacity_available() { /* ... */ }

#[tokio::test]
async fn storage_writer_reports_queue_full_without_blocking() { /* ... */ }

#[tokio::test]
async fn storage_writer_records_failed_database_writes() { /* ... */ }
```

The fake repository must support a mode that returns `StorageError::InvalidInput("forced failure".to_string())` so DB outage/failure metrics are testable without a real DB.

- [ ] **Step 3: Commit Task 5**

Run:

```bash
git add backend/src/storage/writer.rs backend/tests/storage_writer_tests.rs
git commit -m "feat: add bounded storage writer"
```

## Task 6: Runtime Event Integration And Health Snapshot

**Files:**

- Modify: `backend/src/router.rs`
- Modify: `backend/src/health.rs`
- Modify: `backend/tests/health_tests.rs`

- [ ] **Step 1: Add optional storage writer snapshot to AppState**

Modify `backend/src/router.rs` so `AppState` can later carry a storage writer handle without requiring a database for basic health tests:

```rust
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub storage_writer: Option<StorageWriterHandle>,
}

pub fn build_router(config: AppConfig) -> Router {
    build_router_with_storage(config, None)
}

pub fn build_router_with_storage(
    config: AppConfig,
    storage_writer: Option<StorageWriterHandle>,
) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .with_state(AppState {
            config: Arc::new(config),
            storage_writer,
        })
}
```

- [ ] **Step 2: Extend health response with storage queue when present**

Modify `backend/src/health.rs` to include a nullable `storage_writer` field:

```json
"storage_writer": null
```

or:

```json
"storage_writer": {
  "queued_capacity": 1024,
  "queued_available": 1024,
  "accepted": 0,
  "dropped": 0,
  "written": 0,
  "failed": 0
}
```

- [ ] **Step 3: Update health test**

Modify `backend/tests/health_tests.rs` expected JSON to include `"storage_writer": null`.

- [ ] **Step 4: Commit Task 6**

Run:

```bash
git add backend/src/router.rs backend/src/health.rs backend/tests/health_tests.rs
git commit -m "feat: expose storage writer health snapshot"
```

## Task 7: dev-2 Validation Runbook

**Files:**

- Create: `docs/dev-2-web-6-validation.md`
- Modify: `README.md`
- Modify: `.env.example`

- [ ] **Step 1: Add WEB-6 dev-2 validation doc**

Create `docs/dev-2-web-6-validation.md`:

```markdown
# WEB-6 dev-2 Validation

This validation must run on `dev-2`, not on the local Mac.

## Prepare

```bash
ssh dev-2
cd /opt/polymarket-signal-cockpit
git fetch origin codex/web-6-storage-foundation
git checkout codex/web-6-storage-foundation
```

## Start PostgreSQL

```bash
podman-compose up -d postgres
podman-compose ps
podman-compose logs --tail=80 postgres
```

## Run Tests

```bash
export DATABASE_URL=postgres://polymarket:dev2-local-polymarket-password@127.0.0.1:5432/polymarket
cargo test -p polymarket-backend storage_migrations_tests -- --nocapture
cargo test -p polymarket-backend storage_repository_tests -- --nocapture
cargo test -p polymarket-backend storage_writer_tests -- --nocapture
cargo test -p polymarket-backend
```

## Container Runtime Check

```bash
podman-compose up -d --build backend
curl -fsS http://192.168.103.157:8080/healthz
podman-compose logs --tail=120 backend
```

## Cleanup

If the environment was started only for validation:

```bash
podman-compose down
```

If volumes were created only for disposable validation:

```bash
podman volume ls | grep polymarket
```

Delete only volumes confirmed to belong to this temporary validation run.
```

- [ ] **Step 2: Update README with WEB-6 note**

Add a short section:

```markdown
## WEB-6 Storage Validation

Storage tests and PostgreSQL validation run on `dev-2`; do not start local Mac database containers.
See [docs/dev-2-web-6-validation.md](docs/dev-2-web-6-validation.md).
```

- [ ] **Step 3: Add storage writer env examples**

Add to `.env.example`:

```text
STORAGE_WRITER_QUEUE_CAPACITY=4096
STORAGE_WRITER_FLUSH_INTERVAL_MS=250
```

- [ ] **Step 4: Commit Task 7**

Run:

```bash
git add docs/dev-2-web-6-validation.md README.md .env.example
git commit -m "docs: add web-6 dev2 validation runbook"
```

## Task 8: Full dev-2 Verification And Linear Evidence

**Files:**

- No source files required unless tests reveal bugs.
- Linear issue `WEB-6`.

- [ ] **Step 1: Push branch**

Run:

```bash
git push -u origin codex/web-6-storage-foundation
```

- [ ] **Step 2: Run full verification on dev-2**

Run:

```bash
ssh dev-2 'cd /opt/polymarket-signal-cockpit && git fetch origin codex/web-6-storage-foundation && git checkout codex/web-6-storage-foundation'
ssh dev-2 'cd /opt/polymarket-signal-cockpit && podman-compose up -d postgres'
ssh dev-2 'cd /opt/polymarket-signal-cockpit && export DATABASE_URL=postgres://polymarket:dev2-local-polymarket-password@127.0.0.1:5432/polymarket && cargo test -p polymarket-backend'
ssh dev-2 'cd /opt/polymarket-signal-cockpit && podman-compose up -d --build backend'
ssh dev-2 'curl -fsS http://192.168.103.157:8080/healthz'
ssh dev-2 'cd /opt/polymarket-signal-cockpit && podman-compose logs --tail=120 backend'
```

Expected:

- Rust test suite passes on dev-2.
- Backend container starts.
- `http://192.168.103.157:8080/healthz` returns `status=ok`.
- Health payload includes storage writer field when configured or `null` when not configured.

- [ ] **Step 3: Cleanup temporary resources**

If WEB-6 validation used only temporary containers:

```bash
ssh dev-2 'cd /opt/polymarket-signal-cockpit && podman-compose down'
```

If retaining PostgreSQL/backend as the shared dev-2 environment, record that explicitly in Linear instead of claiming cleanup.

- [ ] **Step 4: Update Linear**

Add a `WEB-6` comment:

```markdown
WEB-6 implementation complete.

Branch: `codex/web-6-storage-foundation`
Commits:
- <commit hashes>

Implemented:
- Initial PostgreSQL schema/migrations.
- Typed storage repository.
- Bounded async storage writer.
- Runtime event persistence.
- Replay tick query by market/window.
- dev-2 validation runbook.

Verification:
- `cargo test -p polymarket-backend` on dev-2: PASS
- `podman-compose up -d --build backend` on dev-2: PASS
- `curl -fsS http://192.168.103.157:8080/healthz`: PASS

Cleanup:
- <containers/volumes retained or cleaned>

Safety:
- No secrets committed.
- No real order execution or private key handling added.
```

- [ ] **Step 5: Move WEB-6 status**

Set Linear `WEB-6` to `Done` only after:

- Tests pass.
- dev-2 runtime/container validation passes.
- Review findings are resolved or documented.
- Temporary resource cleanup/retention is recorded.

## Plan Self-Review

- Spec coverage: The plan covers migrations, typed insert/read APIs, bounded writer, idempotency/dedupe, runtime events, replay query, dev-2 validation, and cleanup.
- Placeholder scan: No TBD/TODO placeholders are present.
- Type consistency: Public storage types are defined before repository/writer tasks use them.
- Scope check: The plan stays inside WEB-6 and does not implement realtime collectors, model plugins, Feishu sending, Web UI, or live trading.
