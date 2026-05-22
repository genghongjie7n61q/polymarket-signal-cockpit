# polymarket-signal-cockpit

Realtime research cockpit for Polymarket crypto Up/Down markets.

The current codebase is a working prototype for:

- BTC 5m and ETH 15m market monitoring.
- Coinbase WebSocket tick ingestion.
- Polymarket market snapshot lookup.
- Backtested model selection.
- Feishu webhook alerts.
- Paper-only order suggestions.

This project does not place real orders. It produces research signals, alerts, and paper order recommendations for manual review.

## Current Prototype

Run BTC 5m monitoring:

```bash
FEISHU_WEBHOOK_URL='https://...' \
python3 scripts/eth_15m_platform.py \
  --market btc5m \
  --auto-current \
  --monitor-ws \
  --loop \
  --quiet \
  --bankroll 15 \
  --alert-file reports/btc_5m_alerts.jsonl \
  --log-file reports/btc_5m_runtime.jsonl
```

Run tests:

```bash
python3 -m unittest tests/test_eth_15m_platform.py
```

## Rust Backend Baseline

Agents must not run project services, Docker, databases, or validation stacks on the local Mac.
Use local checkout only for code editing, Git operations, and lightweight inspection.

Backend tests and runtime validation run on `dev-2`:

```bash
ssh dev-2
cd /opt/polymarket-signal-cockpit
set -a
. ./.env
set +a
podman run --rm --network host \
  -e CARGO_HOME=/cargo \
  -e CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse \
  -e DATABASE_URL="$DATABASE_URL" \
  -v "$HOME/.cache/polymarket-cargo/cargo":/cargo \
  -v "$HOME/.cache/polymarket-cargo/target":/workspace/target \
  -v "$PWD":/workspace \
  -w /workspace \
  rust:1.87-bookworm \
  cargo test -p polymarket-backend --locked
```

The dev-2 deployment reads secrets only from the uncommitted dev-2 `.env` file. Required values include `POSTGRES_PASSWORD`, `ADMIN_API_TOKEN`, and `DATABASE_URL` for one-off test commands. Proxy variables such as `COINBASE_WS_PROXY` and `POLYMARKET_HTTP_PROXY` are operator-provided only; the committed compose file does not default traffic through a proxy.

Configuration write APIs require `Authorization: Bearer <ADMIN_API_TOKEN>`. Read APIs and health endpoints do not expose this token or raw webhook URLs.

## WEB-6 Storage Validation

Storage tests and PostgreSQL validation run on `dev-2`; do not start local Mac database containers. See [docs/dev-2-web-6-validation.md](docs/dev-2-web-6-validation.md).

## Target Architecture

See [docs/architecture.md](docs/architecture.md) for the proposed platform architecture and [docs/model-plugin-api.md](docs/model-plugin-api.md) for the model plugin contract.
