# Model Plugin API

The model interface must stay small. A model should not know about Feishu, database tables, or UI details. It receives normalized market context and emits a signal.

## Lifecycle

```text
init(config) -> state
on_tick(context, tick, state) -> ModelDecision
backtest(dataset, config) -> BacktestResult
```

## Context

```json
{
  "market": "btc5m",
  "symbol": "BTC-USD",
  "window": {
    "start_ts": 1779330000,
    "end_ts": 1779330300,
    "elapsed_ms": 182000
  },
  "latest_tick": {
    "source": "coinbase",
    "ts_ms": 1779330182123,
    "price": 77998.25,
    "size": 0.03
  },
  "candles": [
    {
      "start_ts": 1779330000,
      "open": 77965.3,
      "high": 78010.1,
      "low": 77920.4,
      "close": 77998.2,
      "volume": 12.3
    }
  ],
  "polymarket": {
    "event_slug": "btc-updown-5m-1779330000",
    "up_price": 0.505,
    "down_price": 0.495,
    "spread": 0.01,
    "liquidity": 12225.36
  },
  "portfolio": {
    "bankroll": 15.0,
    "max_signal_size": 1.5
  }
}
```

## Decision Output

```json
{
  "action": "NO_TRADE",
  "reason": "waiting for decision minute"
}
```

```json
{
  "action": "CANDIDATE",
  "side": "Up",
  "confidence": 0.9091,
  "limit_price": 0.505,
  "suggested_size": 1.22,
  "ttl_ms": 15000,
  "reason": "m3 move crossed 4bps threshold",
  "features": {
    "decision_return": 0.0004767,
    "threshold_bps": 4
  }
}
```

## Backtest Result

```json
{
  "model_id": "threshold_direction",
  "model_version": "0.1.0",
  "dataset": "btc5m_recent_576",
  "trades": 253,
  "wins": 230,
  "accuracy": 0.9091,
  "wilson_lower_bound": 0.8673,
  "coverage": 0.44,
  "max_drawdown": 0.0,
  "parameters": {
    "decision_minute": 3,
    "threshold_bps": 4
  }
}
```

## Rules

- Model output must be deterministic for the same input.
- Models may keep internal state, but state must be serializable.
- Models must not perform network calls in the realtime path.
- Models must not send notifications directly.
- Models must include a TTL for actionable decisions.
- Every signal must record model id, model version, parameters, and feature values used.
