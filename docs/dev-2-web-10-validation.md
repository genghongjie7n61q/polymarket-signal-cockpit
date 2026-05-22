# WEB-10 dev-2 Validation

Issue: `WEB-10 / M6 飞书通知平台`

Branch: `codex/web-10-feishu-notification-platform`

Base: `feature/rust-backend-baseline`

## Scope

- Feishu interactive card renderer with actionable first-screen content.
- Bounded notification runtime with queue-full behavior, retry, timeout, dedupe,
  and delivery audit writes.
- Signal notification bridge from persisted actionable signals to notifier queue.
- Dry-run REST endpoint for enabled Feishu channels.
- Runtime health surfaces notifier metrics.
- Webhook URLs are masked in API responses and card payloads.

## dev-2 Commands

All commands were executed inside the existing dev-2 backend container after
syncing the branch to `/opt/polymarket-signal-cockpit`.

```bash
cargo fmt --check
cargo test -p polymarket-backend --locked --test notification_feishu_tests
cargo test -p polymarket-backend --locked --test notification_runtime_tests
cargo test -p polymarket-backend --locked --test api_routes_tests --test health_tests
```

Observed results:

- `notification_feishu_tests`: 3 passed.
- `notification_runtime_tests`: 7 passed.
- `storage_writer_tests`: 6 passed.
- `api_routes_tests`: 15 passed.
- `health_tests`: 4 passed.

Full validation before PR:

```bash
cargo fmt --check && cargo test -p polymarket-backend --locked
curl -fsS http://192.168.103.157:8080/healthz
```

Observed full validation:

- `cargo fmt --check && cargo test -p polymarket-backend --locked`: passed.
- Full backend suite: 102 tests passed, 0 failed.
- Existing dev-2 service health on `http://192.168.103.157:8080/healthz`:
  returned `status=ok`, `database_configured=true`, `coinbase=fresh`,
  `polymarket=fresh`.

Branch smoke validation:

- Started a temporary dev-2 container from the synced branch on port `18082`.
- `curl -fsS http://192.168.103.157:18082/healthz` returned
  `status=ok`, `version=0.1.0-web10-smoke`, and
  `notification.task_status=running`.
- Removed the temporary container after the smoke check.

## Operational Notes

- Notification HTTP I/O is performed by `NotificationRuntime`, not by the
  realtime/model critical path.
- Persisted actionable signals are handed off by `StorageWriter` with
  `try_send`; channel loading and Feishu enqueue happen in
  `SignalNotificationBridge`.
- The worker receives jobs through a bounded Tokio channel; queue pressure is
  visible through runtime health and drops quickly when full.
- Real signal notifications write `notification_deliveries` through the storage
  writer. Dry-run attempts write `runtime_events` only, because they must not
  create real signals or orders.
- API responses and Feishu cards use masked webhook URLs. Raw webhooks are not
  returned by config APIs, dry-run APIs, health endpoints, or card JSON.
