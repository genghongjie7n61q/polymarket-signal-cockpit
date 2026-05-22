# WEB-9 Replay And Backtesting Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a deterministic replay/backtesting foundation that proves whether a market/model assignment is eligible for production alerts.

**Architecture:** Add a pure `backend/src/backtest` module for replay inputs, first-actionable-alert freeze semantics, outcomes, and metrics. Persist aggregate backtest runs through the existing storage repository, expose recent runs via `/api/backtests`, and keep per-window replay outputs in run metrics JSON for this milestone so the schema stays small while the semantics are testable.

**Tech Stack:** Rust, Axum, SQLx/PostgreSQL, `bigdecimal`, existing model/storage/API patterns. All verification runs on `dev-2`; local Mac is only for editing, Git, and lightweight inspection.

---

## File Structure

- Create `backend/src/backtest/mod.rs`: public module exports.
- Create `backend/src/backtest/types.rs`: replay input, frozen alert, per-window result, aggregate summary, eligibility decision.
- Create `backend/src/backtest/metrics.rs`: Wilson lower bound, drawdown, calibration buckets, deterministic aggregate calculations.
- Create `backend/src/backtest/engine.rs`: deterministic replay over windows and first actionable alert freeze.
- Modify `backend/src/lib.rs`: export `backtest`.
- Modify `backend/src/storage/types.rs`: add `NewBacktestRun`, `BacktestRunRecord`.
- Modify `backend/src/storage/repository.rs`: add `insert_backtest_run`, `latest_backtest_runs`.
- Modify `backend/src/api/dto.rs`: add backtest response DTOs.
- Modify `backend/src/api/routes.rs`: add `GET /api/backtests`.
- Modify `backend/tests/api_routes_tests.rs`: add route tests and fake repository methods.
- Modify `backend/tests/storage_repository_tests.rs`: add persistence test.
- Create `backend/tests/backtest_metrics_tests.rs`: pure metrics tests.
- Create `backend/tests/backtest_engine_tests.rs`: replay/freeze/eligibility tests.
- Modify `docs/model-plugin-api.md`: document production eligibility requirement from recent qualifying backtest.
- Add `docs/dev-2-web-9-validation.md`: exact validation evidence.

## Task 1: Pure Backtest Metrics

**Files:**
- Create: `backend/tests/backtest_metrics_tests.rs`
- Create: `backend/src/backtest/mod.rs`
- Create: `backend/src/backtest/types.rs`
- Create: `backend/src/backtest/metrics.rs`
- Modify: `backend/src/lib.rs`

- [ ] **Step 1: Write failing tests**

Add tests for Wilson lower bound, equity drawdown, consecutive losses, EV, coverage, calibration buckets, and BTC/ETH independent parameters:

```rust
use polymarket_backend::backtest::{
    compute_backtest_metrics, BacktestReplayOutput, FrozenActionableAlert,
};
use serde_json::json;
use time::OffsetDateTime;

#[test]
fn metrics_compute_wilson_drawdown_ev_and_calibration() {
    let outputs = vec![
        output("btc5m", "Up", true, "0.62", "0.50", "2.0"),
        output("btc5m", "Down", false, "0.58", "0.50", "2.0"),
        output("btc5m", "Up", true, "0.72", "0.55", "2.0"),
    ];

    let metrics = compute_backtest_metrics("baseline_direction", "0.1.0", "btc5m:3", 5, &outputs, json!({"threshold_bps": 4}));

    assert_eq!(metrics.trades, 3);
    assert_eq!(metrics.wins, 2);
    assert_eq!(metrics.coverage.to_string(), "0.6");
    assert_eq!(metrics.accuracy.to_string(), "0.666666667");
    assert_eq!(metrics.expected_value.to_string(), "0.133333333");
    assert_eq!(metrics.max_consecutive_losses, 1);
    assert_eq!(metrics.calibration_buckets.len(), 2);
    assert!(metrics.wilson_lower_bound.to_string().starts_with("0.20"));
    assert_eq!(metrics.parameters, json!({"threshold_bps": 4}));
}

#[test]
fn metrics_keep_btc_and_eth_parameter_sets_independent() {
    let btc = compute_backtest_metrics("baseline_direction", "0.1.0", "btc5m:1", 1, &[output("btc5m", "Up", true, "0.70", "0.50", "1.0")], json!({"threshold_bps": 3}));
    let eth = compute_backtest_metrics("baseline_direction", "0.1.0", "eth15m:1", 1, &[output("eth15m", "Down", true, "0.80", "0.55", "1.0")], json!({"threshold_bps": 8}));

    assert_eq!(btc.market_key, "btc5m");
    assert_eq!(eth.market_key, "eth15m");
    assert_eq!(btc.parameters, json!({"threshold_bps": 3}));
    assert_eq!(eth.parameters, json!({"threshold_bps": 8}));
}
```

- [ ] **Step 2: Verify RED on dev-2**

Run:

```bash
rsync -az --exclude .git --exclude target --exclude .env --exclude .worktrees ./ dev-2:/opt/polymarket-signal-cockpit/
ssh dev-2 'podman exec polymarket-signal-cockpit_backend_1 sh -lc '\''export PATH=/usr/local/cargo/bin:/usr/local/rustup/toolchains/1.87.0-x86_64-unknown-linux-gnu/bin:$PATH; cd /workspace; cargo test -p polymarket-backend --locked backtest_metrics_tests'\'''
```

Expected: FAIL because `polymarket_backend::backtest` does not exist.

- [ ] **Step 3: Implement minimal metrics module**

Define `FrozenActionableAlert`, `BacktestReplayOutput`, `CalibrationBucket`, `BacktestMetrics`, and `compute_backtest_metrics`. Use decimal strings rounded to 9 fractional digits for stable API/test output. Profit per winning share is `(1 - price_paid) * size`; loss is `price_paid * size`; EV is mean profit/loss per trade.

- [ ] **Step 4: Verify GREEN on dev-2**

Run the same targeted command. Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add backend/src/lib.rs backend/src/backtest backend/tests/backtest_metrics_tests.rs
git commit -m "feat: add backtest metrics core"
```

## Task 2: Replay Engine And First Alert Freeze

**Files:**
- Create: `backend/tests/backtest_engine_tests.rs`
- Create: `backend/src/backtest/engine.rs`
- Modify: `backend/src/backtest/mod.rs`
- Modify: `backend/src/backtest/types.rs`

- [ ] **Step 1: Write failing tests**

Add tests proving deterministic replay, first actionable alert freeze, no-trade handling, and production eligibility threshold:

```rust
use polymarket_backend::backtest::{
    replay_backtest, BacktestEligibilityPolicy, ReplayWindow, ReplayWindowCandidate,
};
use serde_json::json;
use time::{Duration, OffsetDateTime};

#[test]
fn replay_freezes_first_actionable_alert_per_window() {
    let start = OffsetDateTime::UNIX_EPOCH;
    let windows = vec![ReplayWindow {
        market_key: "btc5m".to_string(),
        window_start: start,
        window_end: start + Duration::minutes(5),
        final_price: "101".parse().unwrap(),
        open_price: "100".parse().unwrap(),
        candidates: vec![
            candidate(start + Duration::seconds(90), "Down", "0.57", "0.50", "first"),
            candidate(start + Duration::seconds(180), "Up", "0.90", "0.50", "later-better"),
        ],
    }];

    let result = replay_backtest("baseline_direction", "0.1.0", "btc5m:test", json!({"threshold_bps": 4}), &windows);

    assert_eq!(result.replay_outputs.len(), 1);
    assert_eq!(result.replay_outputs[0].alert.side, "Down");
    assert!(!result.replay_outputs[0].won);
    assert_eq!(result.metrics.trades, 1);
}

#[test]
fn replay_is_deterministic_for_same_inputs() {
    let windows = sample_windows();
    let first = replay_backtest("baseline_direction", "0.1.0", "btc5m:test", json!({"threshold_bps": 4}), &windows);
    let second = replay_backtest("baseline_direction", "0.1.0", "btc5m:test", json!({"threshold_bps": 4}), &windows);
    assert_eq!(first, second);
}

#[test]
fn eligibility_requires_enough_recent_trades_and_wilson_floor() {
    let policy = BacktestEligibilityPolicy {
        min_trades: 20,
        min_wilson_lower_bound: "0.55".parse().unwrap(),
        min_coverage: "0.20".parse().unwrap(),
    };
    let result = replay_backtest("baseline_direction", "0.1.0", "btc5m:test", json!({}), &sample_windows());
    let eligibility = result.metrics.production_eligibility(&policy);
    assert!(!eligibility.eligible);
    assert!(eligibility.reasons.iter().any(|reason| reason.contains("min_trades")));
}
```

- [ ] **Step 2: Verify RED on dev-2**

Run targeted test and confirm missing symbols.

- [ ] **Step 3: Implement engine**

Sort each window's candidates by `created_at`, freeze only the first candidate, decide win from `side == Up && final_price > open_price` or `side == Down && final_price < open_price`, and compute metrics from frozen outputs.

- [ ] **Step 4: Verify GREEN on dev-2**

Run targeted engine tests. Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add backend/src/backtest backend/tests/backtest_engine_tests.rs
git commit -m "feat: add deterministic replay engine"
```

## Task 3: Persist Backtest Runs

**Files:**
- Modify: `backend/src/storage/types.rs`
- Modify: `backend/src/storage/repository.rs`
- Modify: `backend/tests/storage_repository_tests.rs`

- [ ] **Step 1: Write failing repository test**

Add a test that inserts one completed run for `btc5m` and then queries latest runs by market/model:

```rust
#[tokio::test]
async fn repository_inserts_and_lists_backtest_runs() {
    let ctx = TestContext::create().await;
    let storage = PostgresStorage::new(ctx.pool.clone());

    let inserted = storage
        .insert_backtest_run(&NewBacktestRun {
            market_key: ctx.market_key.clone(),
            model_key: format!("baseline-{}", ctx.suffix),
            display_name: "Baseline".to_string(),
            model_version: "0.1.0".to_string(),
            parameters: json!({"threshold_bps": 4}),
            window_start: ctx.window_start,
            window_end: ctx.window_start + Duration::minutes(5),
            metrics: json!({"trades": 3, "wins": 2, "eligible": false}),
            status: "completed".to_string(),
        })
        .await
        .expect("insert backtest run");

    let runs = storage
        .latest_backtest_runs(Some(&ctx.market_key), Some(&format!("baseline-{}", ctx.suffix)), 10)
        .await
        .expect("latest runs");

    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].id, inserted.id);
    assert_eq!(runs[0].metrics["trades"], 3);
    ctx.cleanup().await;
}
```

- [ ] **Step 2: Verify RED on dev-2**

Run the repository test target. Expected: missing repository methods/types.

- [ ] **Step 3: Implement repository methods**

`insert_backtest_run` should upsert model/model_version, insert `backtest_runs` with `finished_at = now()` for completed runs, and return model/market fields joined for UI. `latest_backtest_runs` should clamp limit to 1..100 and support optional `market_key` and `model_key` filters.

- [ ] **Step 4: Verify GREEN on dev-2**

Run targeted repository test. Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add backend/src/storage backend/tests/storage_repository_tests.rs
git commit -m "feat: persist backtest runs"
```

## Task 4: Backtest API For Cockpit

**Files:**
- Modify: `backend/src/api/dto.rs`
- Modify: `backend/src/api/routes.rs`
- Modify: `backend/tests/api_routes_tests.rs`

- [ ] **Step 1: Write failing API test**

Add route test:

```rust
#[tokio::test]
async fn backtests_api_returns_latest_runs_for_market_and_model() {
    let repository = ApiRepository {
        backtest_runs: vec![backtest_run("btc5m", "baseline_direction", "0.1.0")],
        ..Default::default()
    };
    let app = build_test_app_with_repository(repository).await;

    let response = app
        .oneshot(Request::builder().uri("/api/backtests?market_key=btc5m&model_key=baseline_direction&limit=5").body(Body::empty()).expect("request should build"))
        .await
        .expect("request should be handled");

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.expect("body should be readable");
    let json: Value = serde_json::from_slice(&body).expect("response should be json");
    assert_eq!(json["runs"].as_array().expect("runs").len(), 1);
    assert_eq!(json["runs"][0]["market_key"], "btc5m");
    assert_eq!(json["runs"][0]["metrics"]["eligible"], false);
}
```

- [ ] **Step 2: Verify RED on dev-2**

Run targeted API test. Expected: 404 or missing fake repository method.

- [ ] **Step 3: Implement route and DTO**

Add `GET /api/backtests` with optional `market_key`, `model_key`, and `limit`. Validate supported market when provided. Return `BacktestsResponseDto { runs }` without exposing secrets.

- [ ] **Step 4: Verify GREEN on dev-2**

Run targeted API test. Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add backend/src/api backend/tests/api_routes_tests.rs
git commit -m "feat: expose backtest runs API"
```

## Task 5: Documentation And Dev-2 Validation

**Files:**
- Modify: `docs/model-plugin-api.md`
- Create: `docs/dev-2-web-9-validation.md`

- [ ] **Step 1: Update docs**

Document:
- backtest uses first frozen actionable alert per market window;
- production alerts require a recent qualifying run;
- metrics include win rate, Wilson lower bound, coverage, EV, drawdown, consecutive losses, price paid, and calibration buckets;
- BTC 5m and ETH 15m keep independent assignment parameters.

- [ ] **Step 2: Run full verification on dev-2**

Run:

```bash
rsync -az --exclude .git --exclude target --exclude .env --exclude .worktrees ./ dev-2:/opt/polymarket-signal-cockpit/
ssh dev-2 'podman exec polymarket-signal-cockpit_backend_1 sh -lc '\''export PATH=/usr/local/cargo/bin:/usr/local/rustup/toolchains/1.87.0-x86_64-unknown-linux-gnu/bin:$PATH; cd /workspace; cargo fmt --check && cargo test -p polymarket-backend --locked'\'''
ssh dev-2 'curl -fsS http://192.168.103.157:8080/healthz'
```

- [ ] **Step 3: Record evidence**

Add command output summary, health check JSON summary, and cleanup status to `docs/dev-2-web-9-validation.md`.

- [ ] **Step 4: Request review**

Use a separate review subagent to check WEB-9 against Linear acceptance criteria, `AGENT.md`, no live trading behavior, dev-2 evidence, and API/storage boundaries.

- [ ] **Step 5: Commit**

```bash
git add docs/model-plugin-api.md docs/dev-2-web-9-validation.md
git commit -m "docs: record web-9 validation"
```

## Self-Review

- Spec coverage: replay from persisted data is represented by repository query and deterministic replay types; walk-forward foundation is represented by replay windows and dataset naming; first actionable alert freeze is mandatory in engine tests; metrics include win rate, Wilson lower bound, coverage, EV, drawdown, consecutive losses, price paid, and calibration buckets; persistence/API let Web Cockpit query results.
- Placeholder scan: no `TBD`, no open-ended "add tests" without examples, and each task has commands and expected outcomes.
- Type consistency: backtest aggregate types live in `backtest`, persisted records live in `storage`, API DTOs consume storage records, and model plugin docs describe eligibility rather than implementing live trading.
