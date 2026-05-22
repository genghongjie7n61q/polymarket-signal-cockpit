# WEB-8 Model Runtime Contract Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Define a small deterministic model plugin contract, add baseline BTC 5m / ETH 15m models, and wire the runtime enough for platform validation.

**Architecture:** Models are pure Rust components under `backend/src/model` and depend only on normalized realtime types, serializable context, and sizing rules. Realtime execution uses bounded channels and cached assignments; database writes are off the critical path through existing storage writer surfaces or follow-up repository adapters.

**Tech Stack:** Rust 1.87, Tokio bounded channels, serde/serde_json, bigdecimal, sha2 for deterministic input snapshot hashing.

---

## File Structure

- Create `backend/src/model/mod.rs`: public model module exports.
- Create `backend/src/model/types.rs`: stable model-facing structs and trait.
- Create `backend/src/model/sizing.rs`: fractional Kelly sizing engine with clamps.
- Create `backend/src/model/baseline.rs`: deterministic threshold-direction baseline model.
- Create `backend/src/model/registry.rs`: built-in model registry and assignment lookup.
- Create `backend/src/model/runtime.rs`: bounded realtime model worker shell and metrics.
- Modify `backend/src/lib.rs`: export `model`.
- Modify `backend/src/realtime/runtime.rs`: accept optional model fanout after state update.
- Modify `backend/src/storage/types.rs` and `backend/src/storage/repository.rs` only if runtime persistence needs model version/window IDs in this issue.
- Test with focused new files under `backend/tests/model_*_tests.rs`.

## Task 1: Contract Types And Deterministic Snapshot Hash

**Files:**
- Create: `backend/src/model/types.rs`
- Create: `backend/src/model/mod.rs`
- Modify: `backend/src/lib.rs`
- Test: `backend/tests/model_contract_tests.rs`

- [x] **Step 1: Write failing contract tests**

```rust
#[test]
fn model_context_hash_is_stable_for_identical_json() {
    let context = sample_context("btc5m");
    assert_eq!(context.input_snapshot_hash(), context.input_snapshot_hash());
}

#[test]
fn candidate_decision_exposes_replay_fields() {
    let decision = ModelDecision::candidate(ModelCandidate {
        side: ModelSide::Up,
        confidence: bd("0.62"),
        limit_price: bd("0.51"),
        suggested_size: bd("1.25"),
        ttl_ms: 15_000,
        reason: "threshold crossed".to_string(),
        features: serde_json::json!({"return_bps": 8.1}),
    });
    assert!(matches!(decision.action, ModelAction::Candidate));
    assert_eq!(decision.ttl_ms(), Some(15_000));
    assert_eq!(decision.features()["return_bps"], serde_json::json!(8.1));
}
```

- [x] **Step 2: Run RED on dev-2**

Run:

```bash
rsync -az --exclude .git --exclude target --exclude .env --exclude .worktrees ./ dev-2:/opt/polymarket-signal-cockpit/
ssh dev-2 'podman exec polymarket-signal-cockpit_backend_1 sh -lc '\''export PATH=/usr/local/cargo/bin:/usr/local/rustup/toolchains/1.87.0-x86_64-unknown-linux-gnu/bin:$PATH; cd /workspace; cargo test -p polymarket-backend --test model_contract_tests --locked'\'''
```

Expected: compile failure because `polymarket_backend::model` does not exist.

- [x] **Step 3: Implement minimal contract**

Create serializable `ModelContext`, `ModelWindow`, `ModelPortfolio`, `ModelPolymarket`, `ModelAssignment`, `ModelDecision`, `ModelAction`, `ModelSide`, `ModelCandidate`, `BacktestResult`, and `StrategyModel`.

Key contract:

```rust
pub trait StrategyModel: Send + Sync {
    fn key(&self) -> &'static str;
    fn version(&self) -> &'static str;
    fn decide(&self, context: &ModelContext) -> ModelDecision;
}
```

Hash implementation uses `serde_json::to_vec(self)` and SHA-256 hex. Models do not know Feishu, HTTP handlers, or database tables.

- [x] **Step 4: Run GREEN**

Run the same `model_contract_tests`; expected: pass.

## Task 2: Fractional Kelly Sizing Engine

**Files:**
- Create: `backend/src/model/sizing.rs`
- Modify: `backend/src/model/mod.rs`
- Test: `backend/tests/model_sizing_tests.rs`

- [x] **Step 1: Write failing sizing tests**

```rust
#[test]
fn fractional_kelly_sizes_positive_edge_and_clamps_to_caps() {
    let engine = SizingEngine::new(SizingConfig {
        bankroll: bd("15"),
        fraction: bd("0.5"),
        max_size: bd("1.5"),
        min_size: bd("0.5"),
    });
    let size = engine.suggest_size(&bd("0.60"), &bd("0.50")).unwrap();
    assert_eq!(size, bd("1.5"));
}

#[test]
fn fractional_kelly_returns_none_when_edge_is_not_positive() {
    let engine = SizingEngine::default_for_bankroll(bd("15"));
    assert_eq!(engine.suggest_size(&bd("0.50"), &bd("0.55")), None);
}
```

- [x] **Step 2: Run RED on dev-2**

Expected: compile failure because `SizingEngine` is missing.

- [x] **Step 3: Implement sizing**

Use binary contract Kelly fraction:

```text
raw_fraction = (win_probability - contract_price) / (1 - contract_price)
size = bankroll * raw_fraction * fraction
```

Return `None` for invalid prices, probabilities outside `[0,1]`, or non-positive edge. Clamp to `[min_size, max_size]`, but return `None` if the clamped value is below minimum before clamping.

- [x] **Step 4: Run GREEN**

Run `model_sizing_tests`; expected: pass.

## Task 3: Baseline Direction Model

**Files:**
- Create: `backend/src/model/baseline.rs`
- Modify: `backend/src/model/mod.rs`
- Test: `backend/tests/model_baseline_tests.rs`

- [x] **Step 1: Write failing baseline tests**

```rust
#[test]
fn baseline_returns_no_trade_before_decision_offset() {
    let model = BaselineDirectionModel::btc5m_default();
    let mut context = sample_context("btc5m");
    context.window.elapsed_ms = 30_000;
    let decision = model.decide(&context);
    assert_eq!(decision.action, ModelAction::NoTrade);
    assert_eq!(decision.reason, "waiting for decision offset");
}

#[test]
fn baseline_returns_candidate_with_ttl_limit_price_size_and_features() {
    let model = BaselineDirectionModel::btc5m_default();
    let mut context = sample_context("btc5m");
    context.window.elapsed_ms = 180_000;
    context.latest_tick.price = bd("101");
    context.candles[0].open = bd("100");
    context.polymarket.up_price = Some(bd("0.50"));
    let decision = model.decide(&context);
    assert_eq!(decision.action, ModelAction::Candidate);
    assert_eq!(decision.side, Some(ModelSide::Up));
    assert_eq!(decision.limit_price, Some(bd("0.50")));
    assert_eq!(decision.ttl_ms(), Some(15_000));
    assert!(decision.suggested_size.unwrap() > bd("0"));
    assert_eq!(decision.features()["model_key"], serde_json::json!("baseline_direction"));
}
```

- [x] **Step 2: Run RED on dev-2**

Expected: compile failure because baseline model is missing.

- [x] **Step 3: Implement baseline model**

Baseline parameters:

```rust
pub struct BaselineDirectionConfig {
    pub decision_offset_ms: i64,
    pub threshold_bps: BigDecimal,
    pub ttl_ms: i32,
    pub sizing: SizingConfig,
}
```

Decision rule:
- If elapsed time is below `decision_offset_ms`, return `NO_TRADE`.
- Compare `latest_tick.price` against first candle open.
- If absolute move is below threshold, return `NO_TRADE`.
- Positive move maps to `Up`, negative move maps to `Down`.
- Limit price uses corresponding Polymarket side price; missing price returns `NO_TRADE`.
- Confidence is deterministic and capped, starting at `0.50 + abs(return_bps)/10000`.
- Suggested size comes from `SizingEngine`.
- Features include `model_key`, `model_version`, `market_key`, `return_bps`, `threshold_bps`, `elapsed_ms`, `limit_price`, and `input_snapshot_hash`.

- [x] **Step 4: Run GREEN**

Run `model_baseline_tests`; expected: pass.

## Task 4: Registry And Assignment Resolution

**Files:**
- Create: `backend/src/model/registry.rs`
- Modify: `backend/src/model/mod.rs`
- Test: `backend/tests/model_registry_tests.rs`

- [x] **Step 1: Write failing registry tests**

```rust
#[test]
fn built_in_registry_resolves_baseline_assignments_for_both_markets() {
    let registry = ModelRegistry::built_ins();
    let btc = registry.get("baseline_direction", "0.1.0").unwrap();
    let eth = registry.get("baseline_direction", "0.1.0").unwrap();
    assert_eq!(btc.key(), "baseline_direction");
    assert_eq!(eth.version(), "0.1.0");
}

#[test]
fn registry_rejects_unknown_model_version() {
    let registry = ModelRegistry::built_ins();
    assert!(registry.get("baseline_direction", "9.9.9").is_none());
}
```

- [x] **Step 2: Implement registry**

Use `BTreeMap<(String, String), Arc<dyn StrategyModel>>`, and keep registration explicit. Do not load dynamic code in WEB-8.

- [x] **Step 3: Run GREEN**

Run `model_registry_tests`; expected: pass.

## Task 5: Realtime Model Worker Shell

**Files:**
- Create: `backend/src/model/runtime.rs`
- Modify: `backend/src/model/mod.rs`
- Modify: `backend/src/realtime/runtime.rs`
- Test: `backend/tests/model_runtime_tests.rs`

- [ ] **Step 1: Write failing runtime tests**

```rust
#[tokio::test]
async fn model_runtime_processes_tick_after_state_owner_and_drops_without_blocking() {
    let registry = ModelRegistry::built_ins();
    let runtime = ModelRuntime::spawn_for_tests(registry, 1);
    runtime.try_enqueue(sample_context("btc5m")).unwrap();
    assert_eq!(runtime.snapshot().accepted, 1);
}
```

- [ ] **Step 2: Implement worker shell**

`ModelRuntime` owns a bounded channel of `ModelContext`. The worker runs `StrategyModel::decide` synchronously and records metrics: accepted, dropped, processed, candidates, no_trades, failed. It does not perform network I/O and does not call Feishu/HTTP.

- [ ] **Step 3: Run GREEN**

Run `model_runtime_tests`; expected: pass.

## Task 6: Documentation And Dev-2 Validation

**Files:**
- Modify: `docs/model-plugin-api.md`
- Create: `docs/dev-2-web-8-validation.md`
- Test: full backend suite on dev-2

- [ ] **Step 1: Keep model API one page**

Update `docs/model-plugin-api.md` to match implemented field names, emphasizing:
- Deterministic model execution.
- No realtime network I/O.
- No dependency on Feishu, HTTP handlers, or database tables.
- Sizing is platform-provided.

- [ ] **Step 2: Run full verification on dev-2**

```bash
rsync -az --exclude .git --exclude target --exclude .env --exclude .worktrees ./ dev-2:/opt/polymarket-signal-cockpit/
ssh dev-2 'podman exec polymarket-signal-cockpit_backend_1 sh -lc '\''export PATH=/usr/local/cargo/bin:/usr/local/rustup/toolchains/1.87.0-x86_64-unknown-linux-gnu/bin:$PATH; cd /workspace; cargo fmt --check; cargo test -p polymarket-backend --locked'\'''
ssh dev-2 'curl -fsS http://192.168.103.157:8080/healthz'
```

Expected: formatting clean, full backend tests pass, health endpoint ok.

## Self Review

- Spec coverage: Tasks cover contract, deterministic decisions, BTC/ETH built-in baseline availability, sizing, registry, runtime shell, docs, and dev-2 validation.
- Deferred intentionally: dynamic WASM/community upload, production notification dispatch, and real order execution are outside WEB-8.
- Risk note: runtime persistence may need a follow-up repository adapter for active assignment IDs and market window IDs if Task 5 remains a shell; create a Linear follow-up only if the implementation cannot persist signals without blocking the realtime path.
