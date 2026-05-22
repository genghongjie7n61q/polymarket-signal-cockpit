# WEB-10 Feishu Notification Platform Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver actionable Feishu notifications for frozen candidate signals without blocking realtime ingestion, model evaluation, or storage writes.

**Architecture:** Add a dedicated notification module with a bounded queue, card renderer, HTTP sender trait, retry/dedupe logic, and storage audit writes through the existing `StorageWriterHandle`. The realtime/model path only enqueues notification work after a persisted actionable signal exists; Feishu HTTP I/O happens in the notifier worker. Dry-run API builds/sends a card without creating a live signal.

**Tech Stack:** Rust, Tokio bounded channels, Reqwest with rustls, existing Axum API, existing PostgreSQL storage repository and `notification_deliveries`.

---

## File Structure

- Create `backend/src/notification/mod.rs`: module exports.
- Create `backend/src/notification/types.rs`: `NotificationJob`, `NotificationResult`, `NotificationRuntimeSnapshot`, retry config.
- Create `backend/src/notification/feishu.rs`: Feishu interactive card payload renderer and webhook sender.
- Create `backend/src/notification/runtime.rs`: bounded worker, dedupe, retry, timeout, audit persistence.
- Modify `backend/src/lib.rs`: export `notification`.
- Modify `backend/src/router.rs`: add optional `notification` runtime to `AppState`.
- Modify `backend/src/main.rs`: spawn notifier when storage is configured.
- Modify `backend/src/api/dto.rs` and `backend/src/api/routes.rs`: add dry-run endpoint and runtime health surface.
- Modify tests and docs listed below.

## Task 1: Feishu Card Payload Renderer

**Files:**
- Create: `backend/tests/notification_feishu_tests.rs`
- Create: `backend/src/notification/mod.rs`
- Create: `backend/src/notification/types.rs`
- Create: `backend/src/notification/feishu.rs`
- Modify: `backend/src/lib.rs`

- [ ] **Step 1: Write failing tests**

Cover:
- Card body contains readable first-screen text: direction, limit price, suggested size, market/window, TTL, confidence.
- Secondary analysis is present but visually after key fields.
- Payload does not include raw webhook URL.

Run on dev-2:

```bash
rsync -az --exclude .git --exclude target --exclude .env --exclude .worktrees ./ dev-2:/opt/polymarket-signal-cockpit/
ssh dev-2 'podman exec polymarket-signal-cockpit_backend_1 sh -lc '\''export PATH=/usr/local/cargo/bin:/usr/local/rustup/toolchains/1.87.0-x86_64-unknown-linux-gnu/bin:$PATH; cd /workspace; cargo test -p polymarket-backend --locked --test notification_feishu_tests'\'''
```

Expected: FAIL because notification module does not exist.

- [ ] **Step 2: Implement renderer**

Use Feishu `interactive` card JSON. Prefer plain text fields in the first elements:

```text
BTC 5m / Up
Limit 0.52 | Size 1.50 | TTL 15s
Confidence 0.68 | Window 2026-05-22T...
```

Keep webhook URL only in `NotificationChannelRecord`, never inside card JSON.

- [ ] **Step 3: Verify GREEN and commit**

Run target test on dev-2. Commit:

```bash
git add backend/src/lib.rs backend/src/notification backend/tests/notification_feishu_tests.rs
git commit -m "feat: render feishu notification cards"
```

## Task 2: Bounded Notification Worker

**Files:**
- Create: `backend/tests/notification_runtime_tests.rs`
- Create: `backend/src/notification/runtime.rs`
- Modify: `backend/src/notification/types.rs`

- [ ] **Step 1: Write failing tests**

Cover:
- `try_enqueue` returns queue-full quickly and increments dropped.
- Worker sends to multiple enabled webhook channels for the signal market.
- Worker retries transient sender failures with bounded attempts and timeout.
- Worker writes `NotificationDelivery` audit records through `StorageWriterHandle`.
- Same frozen signal/channel dedupe key is not resent repeatedly.

- [ ] **Step 2: Implement runtime**

Use:
- `mpsc::channel<NotificationJob>(capacity)`;
- injectable `NotificationSender` trait for tests;
- dedupe key format `signal_id + channel_id` or existing domain key where signal id is not available;
- storage audit via `StorageCommand::NotificationDelivery`;
- runtime snapshot with accepted/dropped/sent/failed/retried/deduped.

Do not perform HTTP or DB work on the model runtime task.

- [ ] **Step 3: Verify GREEN and commit**

Run targeted notification runtime tests on dev-2 and commit:

```bash
git add backend/src/notification backend/tests/notification_runtime_tests.rs
git commit -m "feat: add bounded notification runtime"
```

## Task 3: Wire Runtime And Dry-Run API

**Files:**
- Modify: `backend/src/router.rs`
- Modify: `backend/src/main.rs`
- Modify: `backend/src/api/dto.rs`
- Modify: `backend/src/api/routes.rs`
- Modify: `backend/tests/api_routes_tests.rs`
- Modify: `backend/tests/health_tests.rs`

- [ ] **Step 1: Write failing API tests**

Cover:
- `POST /api/notifications/feishu/dry-run` requires admin token.
- Dry-run returns masked channel and card summary; no raw webhook URL in response.
- Dry-run uses configured channel(s) for the market.
- Runtime health includes notifier queue metrics.

- [ ] **Step 2: Implement wiring**

Add `notification: Option<NotificationRuntime>` to `AppState`. Spawn in `main` only when storage is configured. The dry-run endpoint should:
- validate market;
- load enabled Feishu channels;
- render/send a test card with clear content;
- record delivery attempts if sending occurs;
- never create a real signal.

- [ ] **Step 3: Verify GREEN and commit**

Run target API/health tests on dev-2 and commit:

```bash
git add backend/src/router.rs backend/src/main.rs backend/src/api backend/tests/api_routes_tests.rs backend/tests/health_tests.rs
git commit -m "feat: expose feishu dry-run notifications"
```

## Task 4: Docs, Dev-2 Validation, Review

**Files:**
- Modify: `docs/architecture.md`
- Create: `docs/dev-2-web-10-validation.md`

- [ ] **Step 1: Document operations**

Document:
- notifier is off critical realtime path;
- dedupe and retry semantics;
- dry-run endpoint;
- webhook masking/no secret logging rule.

- [ ] **Step 2: Full dev-2 verification**

Run:

```bash
rsync -az --exclude .git --exclude target --exclude .env --exclude .worktrees ./ dev-2:/opt/polymarket-signal-cockpit/
ssh dev-2 'podman exec polymarket-signal-cockpit_backend_1 sh -lc '\''export PATH=/usr/local/cargo/bin:/usr/local/rustup/toolchains/1.87.0-x86_64-unknown-linux-gnu/bin:$PATH; cd /workspace; cargo fmt --check && cargo test -p polymarket-backend --locked'\'''
ssh dev-2 'curl -fsS http://192.168.103.157:8080/healthz'
```

- [ ] **Step 3: Review**

Request Hooke review for:
- no blocking HTTP on realtime/model/storage paths;
- card text is actionable and not title-only;
- webhook secrets are masked in API/logs;
- retry/dedupe cannot spam;
- dev-2 evidence is fresh.

- [ ] **Step 4: Commit/PR**

Commit docs and create PR to `feature/rust-backend-baseline`. Merge only after reviewer approval.

## Self-Review

- Spec coverage: renderer, multi-webhook worker, retry/timeout/dedupe, delivery persistence, dry-run API, and dev-2 verification are all mapped to tasks.
- Safety: no live trading/private keys/geo-bypass; webhook URLs stay out of card payloads and API responses.
- Performance: queue boundary keeps notification HTTP I/O off the realtime/model critical path.
