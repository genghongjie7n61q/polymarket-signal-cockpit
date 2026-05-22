# WEB-20 Backend API And Config Surface Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the backend API and configuration surface needed by the Web Cockpit without coupling UI code to internal realtime/storage modules.

**Architecture:** Add a focused `api` module with DTOs, REST handlers, and a lightweight WebSocket snapshot stream. Keep DTOs separate from domain/storage types. Use the existing realtime state owner for current live state, and extend storage read APIs only where persistent config/history is required.

**Tech Stack:** Rust, Axum REST routes, Axum WebSocket, Tokio, serde DTOs, sqlx/PostgreSQL, existing realtime runtime and storage repository.

---

### Task 1: API DTOs And Read-Only Runtime Routes

**Files:**
- Create: `backend/src/api/mod.rs`
- Create: `backend/src/api/dto.rs`
- Create: `backend/src/api/routes.rs`
- Modify: `backend/src/lib.rs`
- Modify: `backend/src/router.rs`
- Modify: `backend/src/realtime/runtime.rs`
- Test: `backend/tests/api_routes_tests.rs`

- [x] **Step 1: Write failing tests**

Add tests for:
- `GET /api/markets` returns supported markets with current live state when realtime is configured.
- `GET /api/markets/btc5m/state` returns latest tick, current window, latest Polymarket snapshot, and recent candles.
- `GET /api/runtime/health` returns the same realtime/storage status shape used by `/healthz`.
- Unsupported market keys return `404`.

Run on dev-2:

```bash
podman run --rm --network host \
  -e CARGO_HOME=/cargo \
  -e CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse \
  -e DATABASE_URL=postgres://polymarket:dev2-local-polymarket-password@127.0.0.1:15432/polymarket \
  -v "$HOME/.cache/polymarket-cargo/cargo":/cargo \
  -v "$HOME/.cache/polymarket-cargo/target":/workspace/target \
  -v "$PWD":/workspace -w /workspace \
  rust:1.87-bookworm \
  cargo test -p polymarket-backend --test api_routes_tests --locked
```

Expected before implementation: compile failure because `api` routes do not exist.

- [ ] **Step 2: Implement DTOs and route wiring**

Add DTOs:
- `MarketSummaryDto { market_key, symbol, interval_seconds, source_status, current_window, latest_tick, latest_snapshot }`
- `MarketStateDto { market_key, latest_tick, current_window, latest_snapshot, recent_candles }`
- `RuntimeHealthDto { storage_writer, realtime }`

Add routes:
- `GET /api/markets`
- `GET /api/markets/:market_key/state`
- `GET /api/runtime/health`

Expose `RealtimeRuntime::state_snapshot()` for handlers to read the current state without mutating it.

- [x] **Step 3: Verify green**

Run the same dev-2 test command and then full backend tests.

### Task 2: Persistent Read APIs For Candles And Signals

**Files:**
- Modify: `backend/src/storage/repository.rs`
- Modify: `backend/src/storage/types.rs`
- Modify: `backend/src/api/routes.rs`
- Test: `backend/tests/storage_repository_tests.rs`
- Test: `backend/tests/api_routes_tests.rs`

- [x] **Step 1: Write failing tests**

Add repository tests for:
- Query recent 1m candles by `market_key` and limit.
- Query latest signals by `market_key` and limit.

Add API tests for:
- `GET /api/markets/btc5m/candles?limit=60`
- `GET /api/signals?market_key=btc5m&limit=20`

- [x] **Step 2: Implement storage read methods**

Extend `StorageRepository` with:
- `recent_candles(market_key, limit) -> Vec<CandleRecord>`
- `latest_signals(market_key, limit) -> Vec<SignalRecord>`

Wire handlers to return empty arrays when storage is unavailable, but keep runtime health explicit about missing database configuration.

- [x] **Step 3: Verify green**

Run targeted repository/API tests and full backend tests on dev-2.

Evidence on dev-2:
- RED: `cargo test -p polymarket-backend --test storage_repository_tests repository_queries_recent_candles_by_market --locked` failed because `PostgresStorage` lacked `recent_candles` and `latest_signals`.
- GREEN: targeted repository/API tests passed.
- Full: `cargo test -p polymarket-backend --locked` passed in the Rust 1.87 container with `DATABASE_URL` pointed at dev-2 PostgreSQL `127.0.0.1:15432`.

### Task 3: Model Assignment And Notification Channel Config API

**Files:**
- Modify: `backend/src/storage/repository.rs`
- Modify: `backend/src/storage/types.rs`
- Modify: `backend/src/api/dto.rs`
- Modify: `backend/src/api/routes.rs`
- Test: `backend/tests/storage_repository_tests.rs`
- Test: `backend/tests/api_routes_tests.rs`

- [x] **Step 1: Write failing tests**

Test:
- `GET /api/config/model-assignments` lists active assignments.
- `PUT /api/config/model-assignments/:market_key` persists one active assignment and deactivates previous active assignment for that market.
- `GET /api/config/notification-channels?market_key=btc5m` lists channels.
- `POST /api/config/notification-channels` creates or updates a Feishu webhook config.

- [x] **Step 2: Implement repository and handlers**

Add repository methods:
- `list_model_assignments()`
- `set_active_model_assignment(market_key, model_key, version, parameters)`
- `list_notification_channels(market_key)`
- `upsert_notification_channel(market_key, channel_type, name, webhook_url, enabled)`

Do not log webhook URLs. Do not expose secrets in health output.

- [x] **Step 3: Verify green**

Run targeted tests and full backend tests on dev-2.

Evidence on dev-2:
- RED: `cargo test -p polymarket-backend --test api_routes_tests model_assignments_api_lists_active_assignments --locked` failed because config storage types and trait methods were missing.
- GREEN: `cargo test -p polymarket-backend --test api_routes_tests --locked` passed with 10 API tests.
- GREEN: model assignment and notification channel repository tests passed.
- Full: `cargo test -p polymarket-backend --locked` passed in the Rust 1.87 container.

### Task 4: WebSocket Snapshot Stream

**Files:**
- Modify: `backend/Cargo.toml`
- Create: `backend/src/api/ws.rs`
- Modify: `backend/src/api/routes.rs`
- Test: `backend/tests/api_ws_tests.rs`

- [ ] **Step 1: Write failing tests**

Test:
- Connecting to `/api/ws/markets` receives an initial `snapshot` message.
- Publishing a tick into realtime state results in a later snapshot update.
- Unsupported query parameters return a normal close/error payload rather than panicking.

- [x] **Step 2: Implement WebSocket handler**

Enable Axum `ws` feature and add:
- `GET /api/ws/markets`
- Initial snapshot on connect.
- Periodic snapshot tick every 1s as the first implementation.

This keeps model/web fanout decoupled from the realtime ingestion critical path. A later issue can replace polling with a dedicated broadcaster.

- [x] **Step 3: Verify green**

Run WebSocket tests, full backend tests, and dev-2 service smoke.

Evidence on dev-2:
- RED: `cargo test -p polymarket-backend --test api_ws_tests markets_ws_sends_initial_snapshot --locked` failed with HTTP 404 before the route existed.
- GREEN: `cargo test -p polymarket-backend --test api_ws_tests --locked` passed with 3 WebSocket tests.
- Full: `cargo test -p polymarket-backend --locked` passed after enabling Axum `ws` and locking Rust 1.87-compatible transitive dependencies.

### Task 5: dev-2 Runtime Validation And Linear Handoff

**Files:**
- Create: `docs/dev-2-web-20-validation.md`
- Modify: `docs/superpowers/plans/2026-05-22-web-20-backend-api-config.md`

- [x] **Step 1: Deploy on dev-2**

Sync branch to `/opt/polymarket-signal-cockpit`, then restart intentional dev-2 stack:

```bash
POSTGRES_PASSWORD=dev2-local-polymarket-password \
DEV2_APP_PORT=8080 \
podman-compose -f docker-compose.dev2.yml up -d --force-recreate backend
```

- [x] **Step 2: Verify endpoints**

Run:

```bash
curl -fsS http://192.168.103.157:8080/healthz
curl -fsS http://192.168.103.157:8080/api/runtime/health
curl -fsS http://192.168.103.157:8080/api/markets
curl -fsS http://192.168.103.157:8080/api/markets/btc5m/state
```

Record representative output and counters.

- [x] **Step 3: Record handoff**

Update Linear WEB-20 with:
- branch/commit
- tests
- dev-2 runtime evidence
- known gaps
- PR status, including WEB-42 if GitHub PR creation remains blocked.

Evidence recorded in `docs/dev-2-web-20-validation.md`.

### Self-Review Checklist

- [ ] API DTOs do not expose internal storage/realtime structs directly.
- [ ] Handlers do not block realtime ingestion.
- [ ] Webhook URLs are never logged or exposed through health.
- [ ] Missing storage returns clear API behavior.
- [ ] WebSocket implementation is explicit about polling as a first implementation.
- [ ] Unit/API tests and dev-2 validation evidence are recorded before completion.
