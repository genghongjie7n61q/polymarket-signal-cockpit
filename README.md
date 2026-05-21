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

Run backend tests:

```bash
cargo test
```

Run the backend locally:

```bash
POLY_ENV=local \
APP_HOST=127.0.0.1 \
APP_PORT=8080 \
cargo run -p polymarket-backend
```

Check the health endpoint:

```bash
curl -fsS http://127.0.0.1:8080/healthz
```

Local development with containers:

```bash
cp .env.example .env
# Edit .env and replace POSTGRES_PASSWORD / DATABASE_URL with a local random password.
docker compose config
docker compose up --build -d
docker compose ps
docker compose logs --tail=100 backend
curl -fsS http://127.0.0.1:8080/healthz
docker compose down
```

The backend fails fast when `POLY_ENV=production` and `DATABASE_URL` is missing. Real webhook values belong only in local `.env` files and must not be committed.
`BACKEND_BIND` defaults to `127.0.0.1`; set it to `0.0.0.0` only on a controlled deployment host that should accept external traffic.

## WEB-6 Storage Validation

Storage tests and PostgreSQL validation run on `dev-2`; do not start local Mac database containers. See [docs/dev-2-web-6-validation.md](docs/dev-2-web-6-validation.md).

## Target Architecture

See [docs/architecture.md](docs/architecture.md) for the proposed platform architecture and [docs/model-plugin-api.md](docs/model-plugin-api.md) for the model plugin contract.
