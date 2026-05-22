# Web Cockpit API Contract

This document is the frontend-facing contract for WEB-11. It separates stable cockpit API shapes from internal realtime, storage, notification, and model runtime structs.

## Design Status

- Owner issue: WEB-56
- Consumers: Web Cockpit dashboard, market detail panels, model/config controls, Feishu notification settings
- Backend base path: `/api`
- Realtime stream: `/api/ws/markets`
- Contract version: `v0`

## Frontend Screens And Data Needs

| Screen area | Needs | Existing coverage | Gap |
| --- | --- | --- | --- |
| Market overview cards | supported markets, current tick, current window, latest snapshot, source health | `GET /api/markets`, `GET /api/ws/markets` | Add latest signal/backtest/config summary for first paint |
| Market detail | recent candles, latest tick, snapshots, latest signals | `GET /api/markets/{market_key}/state`, `GET /api/markets/{market_key}/candles`, `GET /api/signals` | Signal DTO lacks market/model/window metadata |
| Model selector | active model per market, parameters, version | `GET /api/config/model-assignments`, `PUT /api/config/model-assignments/{market_key}` | Frontend needs typed DTO and clear auth failure behavior |
| Backtest panel | latest backtest runs, key metrics, status | `GET /api/backtests` | Need bootstrap to avoid N+1 first load |
| Notification settings | Feishu channels, masked webhook, enabled state, dry-run result | `GET/POST /api/config/notification-channels`, `POST /api/notifications/feishu/dry-run` | Need latest delivery status for operational visibility |
| Runtime header | realtime/storage/notification health | `GET /api/runtime/health` | Include in bootstrap |

## Stable Identifiers

`market_key` is the frontend routing key. Current supported values:

- `btc5m`
- `eth15m`

The backend may add more markets later, but frontend code must not hard-code only these two except for temporary navigation defaults.

## REST Endpoints

### `GET /api/cockpit/bootstrap`

Purpose: first paint payload for the dashboard. This endpoint aggregates the stable cockpit contract so the frontend does not need to call every read endpoint before rendering.

Response:

```json
{
  "generated_at": "2026-05-22T00:00:00Z",
  "markets": [
    {
      "summary": {
        "market_key": "btc5m",
        "symbol": "BTC-USD",
        "interval_seconds": 300,
        "source_status": "healthy",
        "current_window": null,
        "latest_tick": null,
        "latest_snapshot": null
      },
      "recent_candles": [],
      "active_model": null,
      "latest_signal": null,
      "latest_actionable_alert": null,
      "latest_backtest": null,
      "notification_channels": [],
      "notification_deliveries": []
    }
  ],
  "runtime": {
    "storage_writer": null,
    "realtime": null,
    "notification": null
  }
}
```

Rules:

- `generated_at` is backend UTC time for this snapshot.
- `markets` are sorted by `market_key`.
- `recent_candles` are oldest-to-newest.
- `latest_signal`, `latest_actionable_alert`, and `latest_backtest` are nullable. Missing data is not an error.
- `notification_channels[].webhook_url` is always `null`; only `webhook_url_masked` may be returned.
- The endpoint should return empty arrays when storage is unavailable, while `/api/runtime/health` still exposes missing subsystem status.

### `GET /api/notifications/deliveries?market_key={market_key}&limit={n}`

Purpose: operational view of Feishu delivery state per market.

Response:

```json
{
  "market_key": "btc5m",
  "deliveries": [
    {
      "id": "00000000-0000-0000-0000-000000000000",
      "signal_id": "00000000-0000-0000-0000-000000000000",
      "channel_id": "00000000-0000-0000-0000-000000000000",
      "channel_type": "feishu",
      "channel_name": "primary",
      "status": "sent",
      "attempt_count": 1,
      "response_summary": "ok",
      "created_at": "2026-05-22T00:00:00Z",
      "updated_at": "2026-05-22T00:00:00Z"
    }
  ]
}
```

Rules:

- `limit` defaults to `20` and is clamped to `1..=200`.
- Records are newest-first.
- No webhook URL or secret-like value is returned.
- Supported statuses are currently `sent` and `failed`; frontend should render unknown statuses as neutral text.

### Existing Endpoints Kept Stable

- `GET /api/markets`
- `GET /api/markets/{market_key}/state`
- `GET /api/markets/{market_key}/candles?limit={n}`
- `GET /api/signals?market_key={market_key}&limit={n}`
- `GET /api/backtests?market_key={market_key}&model_key={model_key}&limit={n}`
- `GET /api/config/model-assignments`
- `PUT /api/config/model-assignments/{market_key}`
- `GET /api/config/notification-channels?market_key={market_key}`
- `POST /api/config/notification-channels`
- `POST /api/notifications/feishu/dry-run`
- `GET /api/runtime/health`

## WebSocket Contract

### `GET /api/ws/markets`

Message:

```json
{
  "type": "snapshot",
  "generated_at": "2026-05-22T00:00:00Z",
  "markets": []
}
```

Rules:

- The first message is a `snapshot`.
- Current implementation is periodic. Later broadcaster work may make it event-driven, but the payload shape should remain compatible.
- Unsupported query parameters return:

```json
{
  "type": "error",
  "error": "unsupported_query"
}
```

## TypeScript Contract

The frontend should use generated or manually mirrored types equivalent to:

```ts
export type MarketKey = string;

export interface CockpitBootstrapResponse {
  generated_at: string;
  markets: CockpitMarket[];
  runtime: RuntimeHealth;
}

export interface CockpitMarket {
  summary: MarketSummary;
  recent_candles: Candle[];
  active_model: ModelAssignment | null;
  latest_signal: Signal | null;
  latest_actionable_alert: Signal | null;
  latest_backtest: BacktestRun | null;
  notification_channels: NotificationChannel[];
  notification_deliveries: NotificationDelivery[];
}

export interface NotificationDelivery {
  id: string;
  signal_id: string;
  channel_id: string;
  channel_type: string;
  channel_name: string;
  status: string;
  attempt_count: number;
  response_summary: string | null;
  created_at: string;
  updated_at: string;
}
```

## Implementation Gap List

WEB-56 should close these gaps before WEB-11 UI implementation starts:

1. Add `GET /api/cockpit/bootstrap`.
2. Add `GET /api/notifications/deliveries`.
3. Add a storage repository query for latest notification deliveries by market.
4. Expand route tests so the first-paint contract cannot drift.
5. Add repository tests for delivery status ordering and market filtering.
6. Keep webhook secrets masked or absent in every DTO.

