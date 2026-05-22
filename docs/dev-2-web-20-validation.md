# WEB-20 dev-2 Validation

Validation date: 2026-05-22.

Target host: `dev-2`, health checked through `192.168.103.157`.

## Test Suite

All commands ran on `dev-2` in the Rust 1.87 container with:

```bash
DATABASE_URL=postgres://polymarket:dev2-local-polymarket-password@127.0.0.1:15432/polymarket
```

Results:

- `cargo test -p polymarket-backend --test api_routes_tests --locked`: passed, 10 tests.
- `cargo test -p polymarket-backend --test api_ws_tests --locked`: passed, 3 tests.
- `cargo test -p polymarket-backend --locked`: passed.

## Runtime Deployment

Synced the WEB-20 worktree to `/opt/polymarket-signal-cockpit`, then restarted the intentional dev-2 stack:

```bash
POSTGRES_PASSWORD=dev2-local-polymarket-password \
DEV2_APP_PORT=8080 \
podman-compose -f docker-compose.dev2.yml up -d --force-recreate backend
```

Container status:

- `polymarket-signal-cockpit_postgres_1`: up.
- `polymarket-signal-cockpit_backend_1`: up.

Backend log tail showed startup, migrations, Coinbase collector, Polymarket snapshot refresher, and Axum binding on `0.0.0.0:8080`.

## HTTP Evidence

`curl -fsS http://192.168.103.157:8080/healthz` returned `status=ok`, `database_configured=true`, `storage_writer.task_status=running`, `sources.coinbase=fresh`, `sources.polymarket=fresh`, and `markets_tracked=2`.

Representative counters after startup:

- `realtime.bus.accepted=281`
- `storage_writer.written=243`
- `storage_bridge.missing_market=0`

Endpoint checks:

- `GET /api/runtime/health`: `coinbase=fresh`, `polymarket=fresh`, realtime accepted and storage written counters increasing.
- `GET /api/markets`: returned `btc5m` and `eth15m`, both with live ticks and Polymarket snapshots.
- `GET /api/markets/btc5m/state`: returned latest BTC tick and live candles.
- `GET /api/markets/btc5m/candles?limit=3`: returned an empty persisted candle array; this is expected for now because live candle aggregation is in memory and persisted candle materialization is not part of WEB-20.
- `GET /api/config/notification-channels?market_key=btc5m`: returned zero channels and did not expose raw webhook URLs.

## WebSocket Evidence

A raw WebSocket handshake to `ws://192.168.103.157:8080/api/ws/markets` returned:

- HTTP status: `101 Switching Protocols`
- First frame JSON: `type=snapshot`
- Markets in first frame: `btc5m`, `eth15m`
- BTC latest tick present: yes

## Known Gaps

- `/api/ws/markets` uses 1 second polling snapshots. A future broadcaster can push on state changes without polling.
- Persisted `candles_1m` are readable through API, but this issue does not create a candle materializer from live ticks.
- GitHub PR creation is still blocked by WEB-42 because the connector lacks repository write access.
