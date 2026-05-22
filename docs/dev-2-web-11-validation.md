# WEB-11 Dev-2 Validation

Branch: `codex/web-11-cockpit-dashboard`

## Container Test Evidence

Frontend validation ran on dev-2 in `node:22-bookworm`:

```bash
npm ci && npm test -- --run && npm run build
```

Result:

- 9 Vitest files passed.
- 21 frontend tests passed.
- Vite production build passed.

Backend validation ran on dev-2 in `rust:1.87-bookworm`:

```bash
cargo test -p polymarket-backend --locked
```

Result:

- Full backend suite passed with PostgreSQL migration/repository tests using the temporary dev-2 database.
- Added CORS regression coverage for the Web Cockpit origin.

## Runtime Smoke

Temporary dev-2 compose project: `polymarket_web11`

Ports:

- backend: `192.168.103.157:28080`
- web: `192.168.103.157:25173`
- postgres host port: `127.0.0.1:25432`

Smoke:

```bash
curl -fsS http://192.168.103.157:28080/healthz
curl -fsS http://192.168.103.157:25173
```

Result:

- `/healthz` returned `status=ok`, `database_configured=true`, supported markets `btc5m` and `eth15m`.
- web preview returned the Vite HTML shell.
- Browser loaded the cockpit and fetched bootstrap data after the backend CORS layer was added.
- Review fixes added full bootstrap refresh after WebSocket snapshots, guarded stale WebSocket callbacks, password-style webhook input, and market switch notification state reset.
- Review-fix compose smoke used `npm ci` for the web service startup path and passed on `192.168.103.157:28081` / `192.168.103.157:25174`.

## Browser Findings

Desktop viewport:

- Runtime header, admin token memory-only input, BTC 5m and ETH 15m cards, recommendation panel, chart empty state, and inspector tabs rendered.
- No raw Feishu webhook path was visible.
- No text overlap was observed in the first viewport.

Narrow mobile-sized Chrome window:

- Layout collapsed to one column.
- BTC and ETH market cards stacked without incoherent overlap.
- Recommendation, chart empty state, and inspector tab entry remained reachable below the cards.

## Cleanup

The temporary `polymarket_web11` compose containers were stopped and removed after validation. The temporary PostgreSQL volume was removed.

The review-fix `polymarket_web11_review` compose containers and volume were also stopped and removed.

## Residual Risks

- The validation database had no live ticks, model assignments, signals, backtest rows, or notification rows, so the page displayed empty operational state. Fixture tests cover populated UI states.
- `npm audit` reports 5 moderate vulnerabilities in frontend transitive dependencies. They require a separate dependency review because forced upgrades may be breaking.
- The first web service attempt used port `18080`, which was already occupied on dev-2. Validation used `28080` instead.
- `cargo fmt --check` could not run in the current `rust:1.87-bookworm` container because the `cargo-fmt` component is not installed. Backend compile/tests passed in the same Rust container.
