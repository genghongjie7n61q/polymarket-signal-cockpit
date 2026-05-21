# WEB-7 Realtime Event Bus Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the first reliable realtime market-data path for BTC 5m and ETH 15m: normalize external events, update a single-owner live state, and fan out nonblocking storage/model/web events.

**Architecture:** The backend gets a new `realtime` module with small types, deterministic normalizers, candle/window aggregation, bounded channels, and a state-owner task. Collectors push canonical events into the bus; only the state owner mutates live state; storage and future model/web consumers receive cloned events through bounded fanout.

**Tech Stack:** Rust, Tokio bounded channels, serde JSON parsing, existing WEB-6 storage writer APIs, Axum health endpoint.

---

### Task 1: Realtime Domain Types And Normalizers

**Files:**
- Create: `backend/src/realtime/mod.rs`
- Create: `backend/src/realtime/types.rs`
- Create: `backend/src/realtime/normalize.rs`
- Modify: `backend/src/lib.rs`
- Test: `backend/tests/realtime_normalization_tests.rs`

- [ ] **Step 1: Write failing tests**

Create tests that assert:
- Coinbase ticker JSON for `BTC-USD` becomes a `MarketTick`.
- Invalid product ids are rejected.
- Polymarket snapshot JSON becomes a typed `PolymarketSnapshot`.

Run on dev-2:

```bash
podman run --rm --network polymarket-signal-cockpit_default \
  -e DATABASE_URL=postgres://polymarket:dev2-local-polymarket-password@postgres:5432/polymarket \
  -v "$PWD":/workspace -w /workspace rust:1.87-bookworm \
  cargo test -p polymarket-backend --test realtime_normalization_tests
```

Expected before implementation: compile failures for missing `realtime` module/types.

- [ ] **Step 2: Implement minimal types and parsing**

Add:
- `MarketKey` enum-like string wrapper for `btc5m` and `eth15m`.
- `MarketTick { market_key, symbol, source, source_ts, received_at, price, size, sequence }`.
- `PolymarketSnapshot { market_key, event_slug, captured_at, up_price, down_price, spread, liquidity, payload }`.
- `normalize_coinbase_ticker(payload, received_at) -> Result<MarketTick, RealtimeError>`.
- `normalize_polymarket_snapshot(market_key, payload, captured_at) -> Result<PolymarketSnapshot, RealtimeError>`.

- [ ] **Step 3: Verify green**

Run the same dev-2 command. Expected: normalization tests pass.

### Task 2: Window Rollover And Candle Aggregation

**Files:**
- Create: `backend/src/realtime/window.rs`
- Create: `backend/src/realtime/candle.rs`
- Test: `backend/tests/realtime_state_tests.rs`

- [ ] **Step 1: Write failing tests**

Test:
- BTC 5m timestamp maps to `[floor(ts/300)*300, +300)`.
- ETH 15m timestamp maps to `[floor(ts/900)*900, +900)`.
- Candle aggregation preserves open/high/low/close/volume for multiple ticks in one minute.

Expected before implementation: missing modules/functions.

- [ ] **Step 2: Implement deterministic pure functions**

Add:
- `window_for_tick(market_key, tick_ts) -> MarketWindowState`.
- `CandleAggregator::apply_tick(&MarketTick)`.
- `CandleSnapshot` with `start_ts`, `open`, `high`, `low`, `close`, `volume`.

- [ ] **Step 3: Verify green**

Run:

```bash
podman run --rm --network polymarket-signal-cockpit_default \
  -e DATABASE_URL=postgres://polymarket:dev2-local-polymarket-password@postgres:5432/polymarket \
  -v "$PWD":/workspace -w /workspace rust:1.87-bookworm \
  cargo test -p polymarket-backend --test realtime_state_tests
```

### Task 3: Bounded Event Bus And State Owner

**Files:**
- Create: `backend/src/realtime/bus.rs`
- Create: `backend/src/realtime/state.rs`
- Test: `backend/tests/realtime_bus_tests.rs`

- [ ] **Step 1: Write failing tests**

Test:
- `RealtimeBus::try_publish` returns `QueueFull` and increments dropped count when full.
- `StateOwner` updates only through events read from its channel.
- Tick processing emits storage commands without awaiting database writes.
- Stale source detection reports unhealthy after threshold.

- [ ] **Step 2: Implement bus and state owner**

Add:
- `RealtimeEvent::Tick`, `RealtimeEvent::PolymarketSnapshot`, `RealtimeEvent::SourceHeartbeat`.
- `RealtimeBusHandle` with `try_publish`, `snapshot`.
- `RealtimeStateOwner` that owns `LiveMarketState` for supported markets.
- Nonblocking fanout to `StorageWriterHandle`, using `try_enqueue`.

- [ ] **Step 3: Verify green**

Run `cargo test -p polymarket-backend --test realtime_bus_tests` in the dev-2 Rust container.

### Task 4: Backend Runtime Wiring And Health

**Files:**
- Modify: `backend/src/config.rs`
- Modify: `backend/src/main.rs`
- Modify: `backend/src/router.rs`
- Modify: `backend/src/health.rs`
- Test: `backend/tests/config_tests.rs`
- Test: `backend/tests/health_tests.rs`

- [ ] **Step 1: Write failing tests**

Add config tests for:
- `REALTIME_EVENT_QUEUE_CAPACITY`
- `REALTIME_STALE_AFTER_MS`

Add health test asserting `realtime` status appears when runtime is configured.

- [ ] **Step 2: Wire runtime**

When backend starts:
- Create `RealtimeBus`.
- Start `RealtimeStateOwner`.
- Keep runtime handles in `AppState`.
- Expose queue metrics, source status, and last tick timestamps in `/healthz`.

- [ ] **Step 3: Verify green**

Run full backend tests on dev-2:

```bash
podman run --rm --network polymarket-signal-cockpit_default \
  -e DATABASE_URL=postgres://polymarket:dev2-local-polymarket-password@postgres:5432/polymarket \
  -v "$PWD":/workspace -w /workspace rust:1.87-bookworm \
  cargo test -p polymarket-backend
```

### Task 5: Collector Interfaces And dev-2 Runtime Smoke

**Files:**
- Create: `backend/src/realtime/collector.rs`
- Modify: `backend/Cargo.toml`
- Modify: `README.md`
- Create: `docs/dev-2-web-7-validation.md`
- Test: existing realtime tests plus health runtime smoke.

- [ ] **Step 1: Add collector abstraction**

Define:
- `CollectorEventSink` trait with `try_publish`.
- `CoinbaseCollectorConfig { symbols }`.
- `PolymarketSnapshotRefresherConfig { markets, interval }`.

The first implementation may expose parsing and sink interfaces without starting external infinite loops in tests.

- [ ] **Step 2: dev-2 runtime validation**

Run:

```bash
podman-compose up -d --build backend
curl -fsS http://192.168.103.157:8080/healthz
podman-compose logs --tail=120 backend
```

Expected:
- backend/postgres healthy.
- `/healthz` includes `storage_writer` and `realtime`.
- No real trading code or private key handling exists.

### Self-Review Checklist

- [ ] Every WEB-7 acceptance criterion maps to a task.
- [ ] Tests cover normalization, window rollover, candle aggregation, bounded queue overflow, stale-source status.
- [ ] DB writes are behind `StorageWriterHandle::try_enqueue` and do not block tick state updates.
- [ ] Runtime state is owned by one task.
- [ ] Real external collectors are isolated from pure parsing/state tests.
- [ ] dev-2 validation commands are recorded in Linear.
