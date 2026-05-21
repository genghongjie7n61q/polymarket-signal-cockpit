# Polymarket Signal Cockpit Architecture

## Goals

Build a low-latency, auditable research platform for Polymarket crypto Up/Down markets. The first supported markets are BTC 5m and ETH 15m.

The platform must:

- Ingest live market data from Coinbase and Polymarket WebSocket/HTTP sources.
- Store raw and normalized history for replay, backtesting, and dashboard display.
- Run pluggable strategy models on every relevant tick.
- Broadcast live state to a Web cockpit.
- Send actionable paper-trade recommendations to one or more Feishu webhooks.
- Keep model interfaces simple enough for future community plugins.
- Avoid executing real orders unless a later, explicitly approved trading module is designed separately.

## Recommended Stack

Use a Rust realtime core with a web frontend and PostgreSQL storage.

- Runtime core: Rust, Tokio, Axum.
- Database: PostgreSQL with TimescaleDB extension when available.
- Web API: Axum REST plus WebSocket.
- Frontend: React with Vite or Next.js.
- Charts: lightweight-charts for recent candles and signal overlays.
- Model plugins:
  - Built-in production models as Rust crates.
  - Community plugins as WASM modules.
  - Python plugins only for local research/development mode.
- Deployment: Docker Compose on dev-2 for PostgreSQL, backend, and web.

Rust is recommended for the realtime core because the critical path is event fanout, state updates, model execution, storage queues, WebSocket broadcasts, and notification dispatch under short market windows. Python remains useful for model research, notebooks, and migration of current strategy logic.

## High-Level Components

```text
Market Data Connectors
  -> Normalizer
  -> In-Memory Market State
  -> Strategy Engine
  -> Signal Store
  -> WebSocket Broadcaster
  -> Notification Queue

Storage Writer Queue
  -> PostgreSQL / TimescaleDB

Web Cockpit
  -> REST config API
  -> WebSocket live feed
```

## Critical Data Flow

1. Coinbase tick arrives through WebSocket.
2. The normalizer converts it into a canonical `MarketTick`.
3. The in-memory state updates the current market window immediately.
4. The strategy engine receives the updated context and runs active models.
5. If a model emits `CANDIDATE`, the core refreshes Polymarket price/spread before emitting an actionable alert.
6. The signal is persisted and broadcast to the Web cockpit.
7. Feishu notifications are sent asynchronously.
8. Raw ticks and normalized derived records are queued for storage without blocking the strategy path.

## Fixed vs Changing Recommendations

The platform must distinguish:

- `live_signal`: changes with each tick and is shown in the cockpit.
- `actionable_alert`: first qualifying signal for a market window, frozen for audit/backtest.

This matters because backtests must replay the first actionable alert price, not a later favorable price. Live dashboard suggestions can update, but historical performance accounting must use immutable alert snapshots.

## Storage

PostgreSQL is preferred over JSONL/SQLite for this platform because it needs concurrent reads, historical Web queries, backtest datasets, signal auditability, and future multi-process writers.

Suggested tables:

```text
assets
markets
market_windows
ticks
candles_1m
polymarket_snapshots
models
model_versions
model_assignments
backtest_runs
signals
paper_orders
notification_channels
notification_deliveries
runtime_events
```

Tick storage should be append-only. Signal and alert records should include model version, market window, input snapshot hash, and TTL.

## Concurrency Model

Use async tasks with bounded channels:

- `collector_task`: reads external WebSockets.
- `normalizer_task`: converts raw events.
- `state_task`: owns mutable market state.
- `strategy_task`: runs selected models.
- `storage_writer_task`: batches database writes.
- `notifier_task`: sends Feishu messages with retry and deduplication.
- `websocket_task`: broadcasts live state to web clients.

Only the state task should mutate live market state. Database writes and notification sends must not block strategy evaluation.

## Web Cockpit

Initial cockpit views:

- BTC 5m and ETH 15m cards.
- Current market window countdown.
- Polymarket Up/Down prices and spread.
- Recent candles with signal markers.
- Active model selection per market.
- Current live signal and latest actionable alert.
- Feishu webhook configuration per market, supporting multiple addresses.
- Signal history and notification delivery status.
- Backtest summary: win rate, Wilson lower bound, coverage, drawdown, and last N trades.

## Feishu Notifications

Each market can have multiple webhook channels. Delivery must be asynchronous and deduplicated by:

```text
market_window_id + model_version_id + side + alert_type
```

Notification payload should prioritize:

1. Direction.
2. Limit price.
3. Suggested size.
4. Market/window.
5. TTL.
6. Model and backtest details.

## Risk Points

- Coinbase is a proxy for Chainlink settlement data; backtest/live mismatch can occur.
- Polymarket prices can change between signal generation and notification receipt.
- Five-minute markets leave little time for manual action.
- Overfitting risk is high with short recent windows.
- Community plugins require sandboxing and resource limits.
- Notification failure must not block the realtime pipeline.
- Database backpressure must be visible and bounded.

## Phased Plan

### Phase 1: Platform Skeleton

- Add PostgreSQL schema and migrations.
- Split current prototype into modules.
- Store ticks, snapshots, signals, alerts, and runtime events.
- Build a minimal backend API and live WebSocket feed.

### Phase 2: Web Cockpit

- Implement BTC 5m and ETH 15m dashboard cards.
- Add candles, live signal state, alert history, and webhook configuration.
- Add model selection per market.

### Phase 3: Plugin API

- Define stable model input/output schema.
- Convert current threshold strategy into a built-in model.
- Add model registry and versioned backtest records.

### Phase 4: Hardening

- Add walk-forward backtesting.
- Add replay mode from database.
- Add notification retry/deduplication.
- Add observability: latency, dropped events, queue depth, DB write lag.

### Phase 5: Community Plugins

- Add WASM plugin loader.
- Add plugin package metadata, signatures, and permission model.
- Add sandbox resource limits and compatibility tests.
