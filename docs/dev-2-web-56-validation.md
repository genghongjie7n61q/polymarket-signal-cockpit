# WEB-56 dev-2 Validation

Date: 2026-05-22
Branch: `codex/web-11-api-contract-gap`
Worktree: `/Users/genghongjie/AI/workspaces/polymarket/.worktrees/web-11-api-contract-gap`
dev-2 runtime path: `/opt/polymarket-signal-cockpit`

## Scope

WEB-56 added the frontend-facing Web Cockpit API contract and closed the immediate backend gaps before WEB-11 UI work:

- `GET /api/cockpit/bootstrap`
- `GET /api/notifications/deliveries?market_key=&limit=`
- `StorageRepository::latest_notification_deliveries`
- DTOs for cockpit bootstrap and notification deliveries
- Contract tests for first paint and webhook secret safety

## TDD Evidence

Repository RED:

```text
cargo test -p polymarket-backend --test storage_repository_tests repository_queries_latest_notification_deliveries_by_market --locked

error[E0432]: unresolved import `polymarket_backend::storage::NotificationDeliveryRecord`
error[E0599]: no method named `latest_notification_deliveries` found for struct `PostgresStorage`
```

Repository GREEN:

```text
running 1 test
test repository_queries_latest_notification_deliveries_by_market ... ok
```

API RED:

```text
cargo test -p polymarket-backend --test api_routes_tests --locked

cockpit_bootstrap_api_returns_frontend_contract_without_secrets ... FAILED
notification_deliveries_api_returns_masked_status_for_market ... FAILED

left: 404
right: 200
```

API GREEN:

```text
running 17 tests
test cockpit_bootstrap_api_returns_frontend_contract_without_secrets ... ok
test notification_deliveries_api_returns_masked_status_for_market ... ok
test result: ok. 17 passed; 0 failed
```

Review hardening RED/GREEN:

```text
notification_deliveries_api_returns_masked_status_for_market ... FAILED

left: "failed calling https://open.feishu.cn/open-apis/bot/v2/hook/abcd"
right: "failed calling https://open.feishu.cn/open-apis/bot/v2/hook/****"

notification_deliveries_api_returns_masked_status_for_market ... ok
```

This closes the review note that historical or manually inserted
`response_summary` values could contain a Feishu webhook path.

## Full Test Evidence

Formatting check on changed Rust files:

```text
rustfmt --edition 2024 --check backend/src/api/dto.rs backend/src/api/routes.rs backend/src/storage/repository.rs backend/src/storage/types.rs backend/tests/api_routes_tests.rs backend/tests/storage_repository_tests.rs backend/tests/health_tests.rs backend/tests/storage_writer_tests.rs backend/tests/realtime_storage_bridge_tests.rs backend/tests/notification_runtime_tests.rs
exit code: 0
```

Full backend tests:

```text
cargo test -p polymarket-backend --locked
test result: ok
```

The full backend suite passed in the Rust 1.87 dev-2 container with `DATABASE_URL` pointed at the dev-2 PostgreSQL service.

## Runtime Smoke

Deployment command:

```bash
DEV2_APP_PORT=8080 podman-compose -f docker-compose.dev2.yml up -d --force-recreate backend
```

Health check:

```bash
curl -fsS http://192.168.103.157:8080/healthz
```

Representative result:

```json
{
  "status": "ok",
  "database_configured": true,
  "supported_markets": ["btc5m", "eth15m"],
  "storage_writer": {
    "task_status": "running",
    "dropped": 0,
    "failed": 0
  },
  "realtime": {
    "state": {
      "sources": {
        "coinbase": "fresh",
        "polymarket": "fresh"
      },
      "markets_tracked": 2
    }
  },
  "notification": {
    "task_status": "running"
  }
}
```

Cockpit bootstrap smoke:

```bash
curl -fsS http://192.168.103.157:8080/api/cockpit/bootstrap
```

Summary:

```text
market_count=2
market_keys=["btc5m", "eth15m"]
has_runtime=true
contains_webhook_secret_path=false
```

Notification delivery smoke:

```bash
curl -fsS 'http://192.168.103.157:8080/api/notifications/deliveries?market_key=btc5m&limit=5'
```

Summary:

```text
market_key=btc5m
delivery_count=0
contains_webhook_secret_path=false
```

`delivery_count=0` is acceptable for the current runtime because no actionable Feishu delivery was produced after the container restart.

## Container Status

```text
polymarket-signal-cockpit_postgres_1  Up
polymarket-signal-cockpit_backend_1   Up
```

Backend logs show successful startup and binding to `0.0.0.0:8080`.

## Cleanup

- Test commands used ephemeral `podman run --rm` containers.
- The intentional dev-2 compose stack remains running.
- Temporary synced test directory `/tmp/polymarket-web-56-red` was removed after validation.

## Risks And Follow-ups

- Bootstrap is request-time aggregation. It is fine for WEB-11 first paint but can be cached or backed by a read model if market count grows.
- `/api/ws/markets` still sends periodic market summaries only. A later realtime broadcaster issue should carry latest signal/backtest deltas once the frontend needs lower-latency signal updates.
- Notification delivery endpoint is read-only and secret-safe; delivery rows only appear after persisted actionable signals trigger the notification runtime.
