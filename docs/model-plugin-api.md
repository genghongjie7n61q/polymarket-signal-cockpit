# Model Plugin API

The model interface is intentionally small. A model receives a normalized `ModelContext` and returns a deterministic `ModelDecision`. It must not know about Feishu, HTTP handlers, database tables, browser automation, private keys, or order execution.

## Lifecycle

```text
StrategyModel::key() -> "baseline_direction"
StrategyModel::version() -> "0.1.0"
StrategyModel::decide(context) -> ModelDecision
```

Realtime execution must be synchronous and CPU-only. Network I/O, database reads, notification delivery, and order placement stay outside the model.

## Context

`ModelContext` contains:

- `market_key`, `symbol`, and active `assignment` (`model_key`, `model_version`, `parameters`).
- `window` with `event_slug`, `start_ts`, `end_ts`, and `elapsed_ms`.
- `latest_tick` with source, timestamps, price, size, and sequence.
- recent typed `candles`.
- `polymarket` side prices, spread, liquidity, and event slug.
- `portfolio` sizing inputs such as bankroll and max signal size.

`ModelContext::input_snapshot_hash()` returns a stable SHA-256 hash of the full serialized input. Signals and backtests should store this hash with the decision features.

## Decision

`ModelDecision` is either:

```json
{
  "action": "NO_TRADE",
  "reason": "waiting for decision offset",
  "features": {}
}
```

or:

```json
{
  "action": "CANDIDATE",
  "side": "Up",
  "confidence": "0.60",
  "limit_price": "0.50",
  "suggested_size": "1.50",
  "ttl_ms": 15000,
  "reason": "directional threshold crossed",
  "features": {
    "model_key": "baseline_direction",
    "model_version": "0.1.0",
    "market_key": "btc5m",
    "return_bps": "1000",
    "threshold_bps": "4",
    "input_snapshot_hash": "..."
  }
}
```

## Built-Ins

WEB-8 includes `baseline_direction@0.1.0` for BTC 5m and ETH 15m platform validation. It is not an optimized trading strategy. It waits until the decision offset, compares the latest tick to the first candle open, applies a bps threshold, chooses Up/Down, uses the matching Polymarket side price, and sizes with platform fractional Kelly.

The built-in baseline reads these optional `assignment.parameters` values on every decision:

- `threshold_bps`: positive decimal, default `4`.
- `decision_offset_ms`: non-negative integer, default `180000`.
- `ttl_ms`: positive integer, default `15000`.
- `kelly_fraction`: decimal in `(0, 1]`, default `0.5`.
- `min_size`: positive decimal, default `0.5`.
- `max_size`: positive decimal, default `1.5`, capped by `portfolio.max_signal_size`.

Invalid parameters return `NO_TRADE` with an `invalid assignment parameters: ...` reason instead of panicking.

`SizingEngine` uses binary-contract fractional Kelly:

```text
raw_fraction = (win_probability - contract_price) / (1 - contract_price)
size = bankroll * raw_fraction * fraction
```

Non-positive edge, invalid prices/probabilities, or sizes below the configured minimum produce no candidate.

## Backtest Result

The legacy `model::BacktestResult` is a small model-contract DTO for model/version/result metadata. WEB-9's replay platform uses `backtest::BacktestMetrics` for richer run scoring and persisted Cockpit results. Replay/backtest code should use the same `ModelContext` and `StrategyModel::decide` contract as realtime execution.

WEB-9 adds a replay foundation around that contract. A backtest run freezes the first actionable candidate per market window and scores only that frozen alert. Later candidates in the same window are ignored for scoring, even when they would have performed better, because production users can only act on the first alert they actually received.

Backtest metrics include:

- win rate and Wilson lower bound;
- coverage over replayed windows;
- expected value from realized binary-contract profit/loss;
- max drawdown and max consecutive losses;
- average price paid;
- calibration buckets by confidence range;
- parameters used for the market/model assignment.

BTC 5m and ETH 15m keep independent assignment parameters. A model that has no recent qualifying backtest for the same market key, model key, model version, and parameter set is not eligible for production alerting. The Cockpit can query persisted runs through `GET /api/backtests?market_key=btc5m&model_key=baseline_direction`.

The production eligibility gate checks minimum trades, Wilson lower bound, coverage, minimum expected value, maximum drawdown, maximum consecutive losses, and maximum average price paid. A run stores its own parameter snapshot in `backtest_runs.parameters`; UI/API results must not infer historical run parameters from mutable `model_versions.parameters`.

## Rules

- Same input must produce the same output.
- Models may keep state only if that state is serializable and replayable.
- Models must include TTL for actionable candidates.
- Every candidate must include model key, model version, parameters or feature values, and input snapshot hash.
- Community/dynamic plugin loading, signatures, and sandboxing are future work; WEB-8 only registers explicit built-in Rust models.
