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

## Target Architecture

See [docs/architecture.md](docs/architecture.md) for the proposed platform architecture and [docs/model-plugin-api.md](docs/model-plugin-api.md) for the model plugin contract.
