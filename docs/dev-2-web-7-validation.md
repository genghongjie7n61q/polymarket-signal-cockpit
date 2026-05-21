# WEB-7 dev-2 Validation Notes

Date: 2026-05-21

## Commands

Backend tests:

```bash
podman run --rm --network polymarket-signal-cockpit_default \
  -e CARGO_HOME=/cargo \
  -e CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse \
  -e DATABASE_URL=postgres://polymarket:dev2-local-polymarket-password@postgres:5432/polymarket \
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
- The Coinbase realtime smoke reached `realtime.bus.accepted=245`, `realtime.state.metrics.processed=245`, `sources.coinbase=fresh`, and `markets_tracked=2`.
- The smoke container was removed after validation.

## Network Findings

- Direct dev-2 access to `https://ws-feed.exchange.coinbase.com`, `https://advanced-trade-ws.coinbase.com`, Binance, Kraken, and OKX public endpoints timed out.
- dev-2 host access through clash at `http://127.0.0.1:7890` reached Coinbase.
- A container on `polymarket-signal-cockpit_default` could not reach host-local clash because clash listens on `127.0.0.1`.
- The successful smoke therefore used `--network host` and `COINBASE_WS_PROXY=http://127.0.0.1:7890`.

## Follow-up

- For composed deployment with database and collector together, either expose clash on a container-reachable address or run a dedicated proxy sidecar in the compose network.
- Keep `CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse` and persistent Cargo cache mounts for dev-2 validation.
