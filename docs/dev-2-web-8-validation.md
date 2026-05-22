# dev-2 WEB-8 Validation

Date: 2026-05-22

Target host: `dev-2`, health checked through `192.168.103.157`.

Branch/worktree:

```bash
codex/web-8-model-runtime
```

Baseline:

- Synced the worktree to `/opt/polymarket-signal-cockpit` excluding `.env`, `.git`, `target`, and nested `.worktrees`.
- Ran `cargo test -p polymarket-backend --locked` in the dev-2 Rust container before WEB-8 edits; all backend tests passed.

TDD evidence:

- `model_contract_tests` RED: `polymarket_backend::model` was missing.
- `model_contract_tests` GREEN: 5 passed.
- `model_sizing_tests` RED: `SizingConfig` / `SizingEngine` were missing.
- `model_sizing_tests` GREEN: 4 passed.
- `model_baseline_tests` RED: `BaselineDirectionConfig` / `BaselineDirectionModel` were missing.
- `model_baseline_tests` GREEN: 5 passed.
- `model_registry_tests` RED: `ModelRegistry` was missing.
- `model_registry_tests` GREEN: 3 passed.
- `model_runtime_tests` RED: `ModelRuntime` was missing, then `last_decision` was missing.
- `model_runtime_tests` GREEN: 2 passed.

Runtime boundary:

- Model contract and runtime shell are synchronous and do not call network, Feishu, HTTP handlers, or database tables.
- Realtime state-owner integration is intentionally deferred until a context builder and persistence adapter can be added without putting DB work on the tick critical path.

Final verification commands:

```bash
rsync -az --exclude .git --exclude target --exclude .env --exclude .worktrees ./ dev-2:/opt/polymarket-signal-cockpit/
ssh dev-2 'podman exec polymarket-signal-cockpit_backend_1 sh -lc '\''export PATH=/usr/local/cargo/bin:/usr/local/rustup/toolchains/1.87.0-x86_64-unknown-linux-gnu/bin:$PATH; cd /workspace; cargo fmt --check; cargo test -p polymarket-backend --locked'\'''
ssh dev-2 'curl -fsS http://192.168.103.157:8080/healthz'
```

Final results:

- `cargo fmt --check` passed.
- `cargo test -p polymarket-backend --locked` passed: 85 backend tests, 0 failures.
- `curl -fsS http://192.168.103.157:8080/healthz` returned `status=ok`, `database_configured=true`, and realtime sources `coinbase=fresh`, `polymarket=fresh`.
