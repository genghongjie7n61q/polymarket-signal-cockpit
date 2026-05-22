# WEB-9 Dev-2 Validation

Date: 2026-05-22

Branch: `codex/web-9-replay-backtesting`

## Scope

- Added pure replay/backtest metrics with Wilson lower bound, coverage, EV, drawdown, consecutive losses, price paid, and calibration buckets.
- Added deterministic replay engine that freezes the first actionable alert per market window.
- Added production eligibility policy checks for minimum trades, Wilson lower bound, and coverage.
- Added risk-aware production eligibility checks for minimum EV, maximum drawdown, maximum consecutive losses, and maximum average price paid.
- Added `backtest_runs` repository persistence and `GET /api/backtests`.
- Added `backtest_runs.parameters` snapshot migration so historical runs keep the exact parameter set used for scoring.
- Documented production alert eligibility expectations for recent qualifying backtests.

## Dev-2 Commands

```bash
rsync -az --exclude .git --exclude target --exclude .env --exclude .worktrees ./ dev-2:/opt/polymarket-signal-cockpit/
ssh dev-2 'podman exec polymarket-signal-cockpit_backend_1 sh -lc '\''export PATH=/usr/local/cargo/bin:/usr/local/rustup/toolchains/1.87.0-x86_64-unknown-linux-gnu/bin:$PATH; cd /workspace; cargo fmt --check && cargo test -p polymarket-backend --locked'\'''
ssh dev-2 'curl -fsS http://192.168.103.157:8080/healthz'
```

## Results

- `cargo fmt --check`: passed.
- `cargo test -p polymarket-backend --locked`: passed.
- Backend tests after WEB-9 review fixes: 98 passed, 0 failed.
- New focused tests:
  - `backtest_metrics_tests`: 3 passed.
  - `backtest_engine_tests`: 5 passed.
  - `api_routes_tests`: 12 passed, including `backtests_api_returns_latest_runs_for_market_and_model`.
  - `storage_repository_tests`: 17 passed, including `repository_inserts_and_lists_backtest_runs`, `repository_preserves_backtest_run_parameter_snapshots`, and `repository_backtest_run_does_not_mutate_active_assignment_parameters`.
- `/healthz` via `192.168.103.157:8080`: passed with `status: ok`.
- Runtime health summary:
  - `storage_writer.task_status`: `running`.
  - `storage_writer.dropped`: `0`.
  - `storage_writer.failed`: `0`.
  - `realtime.sources.coinbase`: `fresh`.
  - `realtime.sources.polymarket`: `fresh`.
  - `storage_bridge.missing_market`: `0`.

## Cleanup

No temporary validation containers were created for WEB-9. Validation reused the existing dev-2 backend container.

## Risk Notes

- WEB-9 persists aggregate backtest runs and stores replay outputs in metrics JSON for this milestone; a dedicated per-signal replay table can be added later if UI drill-down needs indexed per-window rows.
- Parameter snapshots are persisted per backtest run. Historical run display and production eligibility must use `backtest_runs.parameters`, not mutable model-version defaults.
- WEB-9 does not add live trading, private key handling, or geo-bypass behavior.
