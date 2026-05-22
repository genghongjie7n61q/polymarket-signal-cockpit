# WEB-56 Web Cockpit API Contract Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Define and implement the stable backend contract the Web Cockpit needs before frontend development proceeds.

**Architecture:** Keep public DTOs in `backend/src/api/dto.rs`, route orchestration in `backend/src/api/routes.rs`, and persistence reads behind `StorageRepository`. Add a first-paint bootstrap endpoint so the UI can render dashboard cards without N+1 calls, and add a notification delivery status endpoint for operational visibility.

**Tech Stack:** Rust, Axum, serde DTOs, sqlx/PostgreSQL, existing realtime runtime, existing storage repository, dev-2 container validation.

---

### Task 1: Contract Documentation

**Files:**
- Create: `docs/web-cockpit-api-contract.md`
- Create: `docs/superpowers/plans/2026-05-22-web-56-web-cockpit-api-contract.md`
- Modify: `docs/architecture.md`

- [x] **Step 1: Document frontend data needs**

Write the cockpit first-paint, WebSocket, model config, backtest, signal, notification, and runtime health data needs in `docs/web-cockpit-api-contract.md`.

- [x] **Step 2: Link architecture**

Add a short API-contract note to `docs/architecture.md` pointing future Web Cockpit workers to `docs/web-cockpit-api-contract.md`.

- [x] **Step 3: Commit contract docs**

Run:

```bash
git diff --check
git add docs/web-cockpit-api-contract.md docs/superpowers/plans/2026-05-22-web-56-web-cockpit-api-contract.md docs/architecture.md
git commit -m "docs: define web cockpit api contract"
```

Expected: no whitespace errors and a docs-only commit.

### Task 2: Notification Delivery Repository Query

**Files:**
- Modify: `backend/src/storage/types.rs`
- Modify: `backend/src/storage/repository.rs`
- Test: `backend/tests/storage_repository_tests.rs`

- [x] **Step 1: Write failing repository test**

Add `repository_queries_latest_notification_deliveries_by_market` to `backend/tests/storage_repository_tests.rs`.

The test inserts:
- one signal for the test market,
- one delivery for the test market channel,
- one unrelated seeded market row is implicitly ignored by querying the test market only.

It then calls:

```rust
let deliveries = storage
    .latest_notification_deliveries(&ctx.market_key, 20)
    .await
    .expect("latest notification deliveries");
```

Expected assertions:

```rust
assert_eq!(deliveries.len(), 1);
assert_eq!(deliveries[0].market_key, ctx.market_key);
assert_eq!(deliveries[0].channel_id, ctx.channel_id);
assert_eq!(deliveries[0].channel_type, "feishu");
assert_eq!(deliveries[0].status, "sent");
assert_eq!(deliveries[0].attempt_count, 1);
assert_eq!(deliveries[0].response_summary.as_deref(), Some("ok"));
```

- [x] **Step 2: Verify RED on dev-2**

Run on dev-2 from the issue worktree copy:

```bash
podman run --rm --network host \
  -e CARGO_HOME=/cargo \
  -e CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse \
  -e DATABASE_URL=$DATABASE_URL \
  -v "$HOME/.cache/polymarket-cargo/cargo":/cargo \
  -v "$HOME/.cache/polymarket-cargo/target":/workspace/target \
  -v "$PWD":/workspace -w /workspace \
  rust:1.87-bookworm \
  cargo test -p polymarket-backend --test storage_repository_tests repository_queries_latest_notification_deliveries_by_market --locked
```

Expected before implementation: compile failure because `latest_notification_deliveries` and `NotificationDeliveryRecord` do not exist.

- [x] **Step 3: Implement storage type and query**

Add `NotificationDeliveryRecord` to `backend/src/storage/types.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, FromRow)]
pub struct NotificationDeliveryRecord {
    pub id: Uuid,
    pub market_key: String,
    pub signal_id: Uuid,
    pub channel_id: Uuid,
    pub channel_type: String,
    pub channel_name: String,
    pub status: String,
    pub attempt_count: i32,
    pub response_summary: Option<String>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}
```

Extend `StorageRepository`:

```rust
async fn latest_notification_deliveries(
    &self,
    market_key: &str,
    limit: i64,
) -> Result<Vec<NotificationDeliveryRecord>, StorageError>;
```

Implement it in `PostgresStorage` with a join from `notification_deliveries` to `signals`, `market_windows`, `markets`, and `notification_channels`, ordered by `nd.updated_at DESC`, limit clamped to `1..=200`.

- [x] **Step 4: Verify GREEN on dev-2**

Run the same targeted repository test. Expected: PASS.

### Task 3: REST Contract Routes

**Files:**
- Modify: `backend/src/api/dto.rs`
- Modify: `backend/src/api/routes.rs`
- Test: `backend/tests/api_routes_tests.rs`

- [x] **Step 1: Write failing route tests**

Add `cockpit_bootstrap_api_returns_frontend_contract_without_secrets`.

Repository fixture:
- one BTC signal,
- one BTC backtest run,
- one BTC model assignment,
- one BTC notification channel,
- one BTC notification delivery.

Call:

```rust
let response = app
    .oneshot(
        Request::builder()
            .uri("/api/cockpit/bootstrap")
            .body(Body::empty())
            .expect("request should build"),
    )
    .await
    .expect("request should be handled");
```

Assert:

```rust
assert_eq!(response.status(), StatusCode::OK);
assert_eq!(json["markets"][0]["summary"]["market_key"], "btc5m");
assert_eq!(json["markets"][0]["active_model"]["model_key"], "baseline");
assert_eq!(json["markets"][0]["latest_signal"]["side"], "Up");
assert_eq!(json["markets"][0]["latest_actionable_alert"]["side"], "Up");
assert_eq!(json["markets"][0]["latest_backtest"]["model_key"], "baseline_direction");
assert_eq!(json["markets"][0]["notification_channels"][0]["webhook_url"], Value::Null);
assert_eq!(json["markets"][0]["notification_deliveries"][0]["status"], "sent");
assert!(!json.to_string().contains("open-apis/bot/v2/hook/abcd"));
```

Add `notification_deliveries_api_returns_masked_status_for_market`.

Call:

```rust
GET /api/notifications/deliveries?market_key=btc5m&limit=20
```

Assert response status `200`, `market_key = "btc5m"`, one delivery, `channel_name = "primary"`, `status = "sent"`, and no webhook URL appears in the response body.

- [x] **Step 2: Verify RED on dev-2**

Run:

```bash
podman run --rm --network host \
  -e CARGO_HOME=/cargo \
  -e CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse \
  -e DATABASE_URL=$DATABASE_URL \
  -v "$HOME/.cache/polymarket-cargo/cargo":/cargo \
  -v "$HOME/.cache/polymarket-cargo/target":/workspace/target \
  -v "$PWD":/workspace -w /workspace \
  rust:1.87-bookworm \
  cargo test -p polymarket-backend --test api_routes_tests cockpit_bootstrap_api_returns_frontend_contract_without_secrets notification_deliveries_api_returns_masked_status_for_market --locked
```

Expected before implementation: route returns `404` or compile fails because DTO methods are missing.

- [x] **Step 3: Implement DTOs**

Add DTOs:

```rust
pub struct CockpitBootstrapDto {
    pub generated_at: time::OffsetDateTime,
    pub markets: Vec<CockpitMarketDto>,
    pub runtime: RuntimeHealthDto,
}

pub struct CockpitMarketDto {
    pub summary: MarketSummaryDto,
    pub recent_candles: Vec<CandleDto>,
    pub active_model: Option<ModelAssignmentDto>,
    pub latest_signal: Option<SignalDto>,
    pub latest_actionable_alert: Option<SignalDto>,
    pub latest_backtest: Option<BacktestRunDto>,
    pub notification_channels: Vec<NotificationChannelDto>,
    pub notification_deliveries: Vec<NotificationDeliveryDto>,
}

pub struct NotificationDeliveriesResponseDto {
    pub market_key: String,
    pub deliveries: Vec<NotificationDeliveryDto>,
}

pub struct NotificationDeliveryDto {
    pub id: uuid::Uuid,
    pub market_key: String,
    pub signal_id: uuid::Uuid,
    pub channel_id: uuid::Uuid,
    pub channel_type: String,
    pub channel_name: String,
    pub status: String,
    pub attempt_count: i32,
    pub response_summary: Option<String>,
    pub created_at: time::OffsetDateTime,
    pub updated_at: time::OffsetDateTime,
}
```

- [x] **Step 4: Implement routes**

Add routes:

```rust
.route("/cockpit/bootstrap", get(cockpit_bootstrap))
.route("/notifications/deliveries", get(latest_notification_deliveries))
```

Bootstrap behavior:
- build market summaries from realtime or configured supported markets,
- fetch recent candles with limit `60`,
- fetch latest signals with limit `20`,
- choose `latest_signal` as first signal,
- choose `latest_actionable_alert` as first signal where `signal_type == "actionable_alert"`,
- choose active model from `list_model_assignments`,
- choose latest backtest from `latest_backtest_runs(Some(market), active_model.model_key, 1)`, falling back to latest any model when no active model exists,
- include notification channels and latest deliveries.

- [x] **Step 5: Verify GREEN on dev-2**

Run targeted API tests. Expected: PASS.

### Task 4: Full Verification And Handoff

**Files:**
- Create: `docs/dev-2-web-56-validation.md`
- Modify: `docs/superpowers/plans/2026-05-22-web-56-web-cockpit-api-contract.md`

- [x] **Step 1: Full backend tests on dev-2**

Run:

```bash
podman run --rm --network host \
  -e CARGO_HOME=/cargo \
  -e CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse \
  -e DATABASE_URL=$DATABASE_URL \
  -v "$HOME/.cache/polymarket-cargo/cargo":/cargo \
  -v "$HOME/.cache/polymarket-cargo/target":/workspace/target \
  -v "$PWD":/workspace -w /workspace \
  rust:1.87-bookworm \
  cargo fmt --check && cargo test -p polymarket-backend --locked
```

Expected: all backend tests pass.

- [x] **Step 2: dev-2 service validation**

Deploy the branch to dev-2 and verify:

```bash
curl -fsS http://192.168.103.157:8080/healthz
curl -fsS http://192.168.103.157:8080/api/cockpit/bootstrap
curl -fsS 'http://192.168.103.157:8080/api/notifications/deliveries?market_key=btc5m&limit=5'
```

Expected:
- `/healthz` is healthy or reports explicit subsystem state,
- bootstrap returns both supported markets,
- delivery endpoint returns no webhook URL.

- [x] **Step 3: Record validation**

Write `docs/dev-2-web-56-validation.md` with:
- branch and commit,
- test commands,
- representative endpoint checks,
- container cleanup status,
- risks and follow-ups.

- [ ] **Step 4: Review, PR, and Linear**

Request a code review subagent against the branch diff. Fix Critical/Important findings, push branch, create PR, and update Linear WEB-56 with validation evidence and the PR URL.

### Self-Review Checklist

- [ ] Contract docs explain which API shapes are stable for WEB-11.
- [ ] Bootstrap does not expose webhook secrets.
- [ ] Notification delivery query is market-scoped and newest-first.
- [ ] Missing storage does not break first paint.
- [ ] Tests run on dev-2, not local Mac.
- [ ] No live trading, private keys, or geo-bypass behavior are introduced.
