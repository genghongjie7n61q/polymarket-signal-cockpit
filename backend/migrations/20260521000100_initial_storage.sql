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
    payload JSONB NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_raw_market_events_idempotency
    ON raw_market_events (source, COALESCE(source_event_id, ''));

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

CREATE UNIQUE INDEX IF NOT EXISTS idx_ticks_idempotency
    ON ticks (market_id, source, source_ts, price, COALESCE(sequence, -1));

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
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_active_model_assignments
    ON model_assignments (market_id)
    WHERE status = 'active';

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

CREATE UNIQUE INDEX IF NOT EXISTS idx_signals_idempotency
    ON signals (
        market_window_id,
        model_version_id,
        signal_type,
        COALESCE(side, ''),
        input_snapshot_hash
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
CREATE INDEX IF NOT EXISTS idx_market_windows_market_start ON market_windows (market_id, start_ts DESC);
CREATE INDEX IF NOT EXISTS idx_signals_window_created ON signals (market_window_id, created_at);
CREATE INDEX IF NOT EXISTS idx_runtime_events_created ON runtime_events (created_at DESC);
