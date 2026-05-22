# dev-2 WEB-49 Validation

Date: 2026-05-22

Target host: `dev-2`, health checked through `192.168.103.157`.

Branch/worktree:

```bash
codex/web-49-assignment-params
```

TDD evidence:

- RED: `cargo test -p polymarket-backend --test model_baseline_tests --locked` failed because baseline ignored `assignment.parameters`.
- GREEN: `model_baseline_tests` passed after adding `BaselineDirectionConfig::from_assignment`.

Implemented parameter surface:

- `threshold_bps`
- `decision_offset_ms`
- `ttl_ms`
- `kelly_fraction`
- `min_size`
- `max_size`

Safety boundary:

- Invalid parameters produce `NO_TRADE` with an `invalid assignment parameters: ...` reason.
- The model still performs no network I/O, database calls, notification delivery, private-key handling, or real order execution.
- Sizing uses `ModelContext.portfolio.bankroll`; parameter `max_size` is capped by `ModelContext.portfolio.max_signal_size`.

Final verification commands:

```bash
rsync -az --exclude .git --exclude target --exclude .env --exclude .worktrees ./ dev-2:/opt/polymarket-signal-cockpit/
ssh dev-2 'podman exec polymarket-signal-cockpit_backend_1 sh -lc '\''export PATH=/usr/local/cargo/bin:/usr/local/rustup/toolchains/1.87.0-x86_64-unknown-linux-gnu/bin:$PATH; cd /workspace; cargo fmt --check && cargo test -p polymarket-backend --locked'\'''
ssh dev-2 'curl -fsS http://192.168.103.157:8080/healthz'
```

Final results:

- `cargo fmt --check` passed.
- `cargo test -p polymarket-backend --locked` passed: 91 backend tests, 0 failures.
- `curl -fsS http://192.168.103.157:8080/healthz` returned `status=ok`, `database_configured=true`, and realtime sources `coinbase=fresh`, `polymarket=fresh`.
