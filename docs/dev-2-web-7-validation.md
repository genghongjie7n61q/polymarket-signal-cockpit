# WEB-7 dev-2 Validation Notes

Date: 2026-05-21, updated 2026-05-22

## Commands

Backend tests:

```bash
podman run --rm --network host \
  -e CARGO_HOME=/cargo \
  -e CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse \
  -e DATABASE_URL=postgres://polymarket:dev2-local-polymarket-password@127.0.0.1:15432/polymarket \
  -v "$HOME/.cache/polymarket-cargo/cargo":/cargo \
  -v "$HOME/.cache/polymarket-cargo/target":/workspace/target \
  -v "$PWD":/workspace \
  -w /workspace \
  rust:1.87-bookworm \
  cargo test -p polymarket-backend --locked
```

Coinbase realtime smoke through dev-2 clash:

```bash
podman run -d --rm \
  --name polymarket-backend-web7-smoke \
  --network host \
  -e CARGO_HOME=/cargo \
  -e CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse \
  -e POLY_ENV=local \
  -e APP_HOST=0.0.0.0 \
  -e APP_PORT=18082 \
  -e APP_VERSION=web7-coinbase-proxy-smoke \
  -e COINBASE_WS_PROXY=http://127.0.0.1:7890 \
  -v "$HOME/.cache/polymarket-cargo/cargo":/cargo \
  -v "$HOME/.cache/polymarket-cargo/target":/workspace/target \
  -v "$PWD":/workspace \
  -w /workspace \
  rust:1.87-bookworm \
  cargo run -p polymarket-backend --locked

curl -fsS http://192.168.103.157:18082/healthz
podman rm -f polymarket-backend-web7-smoke
```

## Results

- `cargo test -p polymarket-backend --locked` passed in the dev-2 Rust 1.87 container.
- `cargo test -p polymarket-backend --locked` passed again after adding the Polymarket Gamma/CLOB snapshot refresher, with `DATABASE_URL` pointed at the dev-2 Postgres port `127.0.0.1:15432`.
- The Coinbase realtime smoke reached `realtime.bus.accepted=245`, `realtime.state.metrics.processed=245`, `sources.coinbase=fresh`, and `markets_tracked=2`.
- The smoke container was removed after validation.
- Read-only Polymarket API checks through clash reached:
  - Gamma event-by-slug: `https://gamma-api.polymarket.com/events/slug/{event_slug}`
  - CLOB prices: `https://clob.polymarket.com/prices`
- After redeploying `docker-compose.dev2.yml`, `curl -fsS http://192.168.103.157:8080/healthz` reported `sources.coinbase=fresh`, `sources.polymarket=fresh`, `realtime.bus.accepted=111`, `realtime.state.metrics.processed=111`, and `storage_writer.written=95`.
- Code review follow-up added regressions for exact Gamma market slug matching and explicit spread preservation; full dev-2 backend tests still passed.

## Network Findings

- Direct dev-2 access to `https://ws-feed.exchange.coinbase.com`, `https://advanced-trade-ws.coinbase.com`, Binance, Kraken, and OKX public endpoints timed out.
- dev-2 host access through clash at `http://127.0.0.1:7890` reached Coinbase.
- A container on `polymarket-signal-cockpit_default` could not reach host-local clash because clash listens on `127.0.0.1`.
- The successful smoke therefore used `--network host` and `COINBASE_WS_PROXY=http://127.0.0.1:7890`.

## Follow-up

- Chosen dev-2 deployment scheme: use `docker-compose.dev2.yml`.
  - `backend` runs with `network_mode: host`, so `COINBASE_WS_PROXY=http://127.0.0.1:7890` and `POLYMARKET_HTTP_PROXY=http://127.0.0.1:7890` reach host-local clash without exposing clash to the LAN.
  - `postgres` remains containerized and publishes only `127.0.0.1:15432`.
  - `DATABASE_URL` points to `127.0.0.1:15432` from the host-network backend.
- Start dev-2 stack:

```bash
POSTGRES_PASSWORD=dev2-local-polymarket-password \
DEV2_APP_PORT=8080 \
podman-compose -f docker-compose.dev2.yml up -d

curl -fsS http://192.168.103.157:8080/healthz
```

- If another backend already occupies port `8080`, validate on a temporary port:

```bash
DEV2_APP_PORT=18082 DEV2_POSTGRES_HOST_PORT=15433 podman-compose -f docker-compose.dev2.yml up -d
curl -fsS http://192.168.103.157:18082/healthz
podman-compose -f docker-compose.dev2.yml down
```

- Keep `CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse` and persistent Cargo cache mounts for dev-2 validation.
