# WEB-11 Strategy Cockpit Design

Date: 2026-05-22

## Product Goal

WEB-11 is not a market-status dashboard. It is the first operator cockpit for a profitable research platform.

Every screen should help answer five questions:

1. Is there a current edge?
2. Which model produced it, with what parameters?
3. What is the suggested action and how long is it valid?
4. What evidence supports it, including recent backtest results?
5. What happened after alerts were sent?

The cockpit remains a research, signal, backtesting, alerting, and paper-trading surface. It must not add real order execution, private key handling, or geo-bypass behavior.

## Current Backend Capability

Available now:

- `GET /api/cockpit/bootstrap`
- `GET /api/ws/markets`
- `GET /api/markets/{market_key}/state`
- `GET /api/markets/{market_key}/candles`
- `GET /api/signals`
- `GET /api/backtests`
- `GET /api/config/model-assignments`
- `PUT /api/config/model-assignments/{market_key}`
- `GET /api/config/notification-channels`
- `POST /api/config/notification-channels`
- `POST /api/notifications/feishu/dry-run`
- `GET /api/notifications/deliveries`
- `GET /api/runtime/health`

Known backend gaps tracked separately:

- `WEB-75 / M7 后端：市场品种配置 API`
- `WEB-76 / M7 后端：手动回测任务 API`

WEB-11 should include UI affordances for market configuration and manual backtesting, but those controls must clearly show pending or disabled states until the APIs exist. The UI must not pretend unavailable actions succeeded.

## Information Architecture

### Desktop

Use a dense cockpit layout:

1. Runtime header
   - WS status.
   - Realtime, storage, notification status.
   - Last bootstrap time and last WS update.
   - Degraded states are prominent.

2. Market cards row
   - One card per supported market.
   - Current first version shows BTC 5m and ETH 15m.
   - Layout must be able to scale to more markets later.

3. Selected market workspace
   - Left: chart and key recommendation detail.
   - Right: strategy inspector tabs.

4. Strategy inspector tabs
   - `Signals`
   - `Backtest`
   - `Strategy Config`
   - `Notifications`
   - `Market Config`

### Mobile

Use a single-market workflow:

1. Sticky runtime strip.
2. Segmented market switcher.
3. One market card.
4. Chart.
5. Collapsible sections for signal, backtest, config, notifications, and market config.

Avoid wide tables on mobile. Use record lists with truncated long fields and expandable details.

## Market Card

Each market card is an operational summary, not a decorative card.

Fields:

- `market_key`
- symbol and interval.
- source status.
- current window countdown.
- window progress.
- latest tick price.
- Polymarket Up price.
- Polymarket Down price.
- spread.
- liquidity when available.
- active model display name, version, and important parameters.
- latest live signal.
- latest actionable alert.
- notification delivery summary.

States:

- loading.
- live.
- stale.
- no window.
- missing Polymarket snapshot.
- no active model.
- no current edge.
- actionable alert available.
- notification failed.

The card must clearly distinguish:

- `live_signal`: changing analysis.
- `actionable_alert`: frozen, auditable recommendation.

## Real-Time Recommendation Panel

This is the highest-priority section.

Show:

- action state: no trade, candidate, actionable alert, expired.
- side: Up or Down.
- confidence.
- limit price.
- suggested size.
- TTL and expiration.
- model key, model version, model parameters.
- reason.
- feature summary.
- input snapshot hash short form.
- created time.

The default view should show the key recommendation fields first. Detailed features and hash can be visually secondary but must be available.

No recommendation should be phrased as guaranteed profit. Use signal language: suggested action, model confidence, edge estimate if present, and risk state.

## Chart Panel

Use `lightweight-charts`.

Render:

- recent candles from bootstrap and candle endpoint.
- latest tick marker when available.
- live signal marker.
- actionable alert marker.

States:

- loading.
- empty candles.
- render error.
- stale data.

The chart must have stable dimensions on desktop and mobile so it does not collapse or resize when data arrives.

## Strategy Config

Each market needs its own model assignment control.

Available now:

- active assignment can be displayed from bootstrap.
- assignment can be saved through `PUT /api/config/model-assignments/{market_key}`.

UI:

- model key input or selector.
- display name.
- version.
- JSON parameter editor for first version.
- save button.
- saving/saved/error status.
- auth error state.

The first version can use a constrained JSON textarea with validation. A later issue can add schema-aware controls per model plugin.

Changing assignment must not imply historical backtest validity. After saving, show a warning or stale badge until a recent backtest exists for the selected model/parameters.

## Backtest Panel

Available now:

- list latest backtest runs through `GET /api/backtests`.
- bootstrap includes latest backtest summary.

Pending backend:

- manual run through `WEB-76`.

UI:

- latest run status.
- model key and version.
- parameters snapshot.
- window start and end.
- win rate.
- Wilson lower bound.
- coverage.
- EV when present.
- drawdown when present.
- eligibility state when present.
- disabled `Run Backtest` button with copy explaining that WEB-76 is pending.

When WEB-76 lands, enable:

- market selection.
- model selection.
- parameter snapshot.
- backtest time window.
- run button.
- job status.

## Market Config Panel

Available now:

- supported markets can be shown from bootstrap.

Pending backend:

- dynamic market config through `WEB-75`.

UI now:

- market key.
- symbol.
- interval.
- source.
- enabled/readiness state.
- disabled add/edit controls with copy explaining that market config API is pending.

When WEB-75 lands, enable:

- create/update market.
- enable/disable.
- event pattern or slug template.
- validation errors.

## Notifications Panel

Available now:

- list channels.
- add/update Feishu webhook channel.
- dry-run enabled channels.
- delivery history.

UI:

- channel list with enabled status.
- masked webhook only.
- add/update webhook form.
- enabled toggle.
- dry-run button.
- latest delivery table or record list.
- failure response summary, already secret-redacted by backend.

The UI must never render raw webhook URLs after a config save. After saving, replace the raw input with masked display plus status.

## Data Flow

First paint:

1. Call `GET /api/cockpit/bootstrap`.
2. Render runtime header, markets, current recommendation, chart seed candles, config state, latest backtest, latest notification delivery.
3. Select the first market by default, preferring `btc5m` when present.

Realtime:

1. Open `GET /api/ws/markets`.
2. Apply snapshot market summaries to existing market state.
3. Mark WS as reconnecting on close/error.
4. Reconnect with bounded backoff.
5. After reconnect, refresh bootstrap to repair drift.

Details:

- Fetch candles when selected market changes.
- Fetch signals for `Signals` tab.
- Fetch backtests for `Backtest` tab.
- Fetch notification channels/deliveries for `Notifications` tab.

Config writes:

- Model assignment writes require admin token.
- Notification writes require admin token.
- WEB-11 should support token configuration through an environment-provided frontend value or a local operator input stored only in browser memory. Do not persist admin token to repository or logs.

## Frontend Tech Choice

Recommended:

- React + Vite + TypeScript.
- Vitest + Testing Library for unit/component tests.
- lightweight-charts for chart rendering.
- lucide-react for icons.
- CSS modules or a small plain CSS structure; avoid adding a heavy UI framework unless needed.

Reason:

- Fast enough for cockpit UI.
- Easy to containerize on dev-2.
- Keeps implementation understandable and testable.

## Deployment Shape

Add a `web` service to dev-2 compose.

Options:

1. Build static frontend and serve it through a small Node/Nginx container.
2. For dev-2 iteration, run Vite preview in a container.

Recommended first version:

- Use Vite dev/preview service in dev-2 for validation.
- Keep backend API URL configurable.
- Later hardening issue can serve static assets through a production image.

No frontend dependency installation or dev server should run on the local Mac.

## Testing And Verification

Unit/component tests:

- bootstrap data maps into two market cards.
- live vs actionable signal labels stay distinct.
- model assignment form validates JSON and handles save errors.
- webhook form does not render raw webhook after save.
- backtest run button is disabled while WEB-76 is pending.
- market config edit controls are disabled while WEB-75 is pending.
- WS reconnect state is shown.

Visual/browser verification on dev-2:

- desktop viewport.
- mobile viewport.
- chart renders nonblank.
- no overlapping text.
- long reason/summary fields truncate or expand cleanly.

Runtime verification:

- `http://192.168.103.157:<web-port>` loads cockpit.
- Browser can reach backend through configured API base.
- `/healthz` remains ok.
- No raw webhook appears in rendered DOM after save/dry-run flows.

## Risks

- Current WS stream only carries market summaries, not full signal/backtest deltas. The UI must refresh details after reconnect or tab interactions.
- Bootstrap is request-time aggregation and can become expensive if markets grow. It is acceptable for the current two-market cockpit.
- Model parameters are free-form JSON in the first version; schema-aware plugin controls should come later.
- Admin token handling needs care. Keep it out of committed files and avoid persistent browser storage in v1.
- Manual backtest and dynamic market config are essential for the full profitability workflow, but their backend APIs are separate issues. WEB-11 must show honest pending states.

## Out Of Scope

- Real order execution.
- Wallet/private key handling.
- Geo-bypass trading behavior.
- Community plugin marketplace UI.
- Full auth/account system.
- Arbitrary market discovery beyond the pending market config API.

## Self-Review

- No placeholders remain.
- Scope is focused on WEB-11 frontend plus disabled affordances for WEB-75 and WEB-76.
- The design supports the profitability goal by foregrounding signal evidence, model config, backtest status, and notification audit.
- The design does not add real trading or secret handling.

