# WEB-11 Strategy Cockpit Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a React strategy cockpit that shows current market state, model recommendations, strategy config, backtest evidence, and notification audit for BTC 5m and ETH 15m.

**Architecture:** Add a focused `web/` Vite React app that consumes the stable WEB-56 API contract. Keep API DTO/types and fetch logic separate from view components. Use bootstrap for first paint, WebSocket snapshots for live market card updates, and focused REST calls for detail tabs and config writes.

**Tech Stack:** React, Vite, TypeScript, Vitest, Testing Library, lightweight-charts, lucide-react, CSS modules/plain CSS, dev-2 Node container for install/test/build/preview.

---

## File Structure

- Create `web/package.json`: frontend scripts and dependencies.
- Create `web/vite.config.ts`: Vite config with test environment.
- Create `web/tsconfig.json`, `web/tsconfig.node.json`: TypeScript config.
- Create `web/index.html`: Vite entry.
- Create `web/src/main.tsx`: React root.
- Create `web/src/App.tsx`: top-level cockpit composition.
- Create `web/src/api/types.ts`: frontend copy of stable backend contract.
- Create `web/src/api/client.ts`: typed REST client.
- Create `web/src/api/ws.ts`: market WebSocket client/reconnect helper.
- Create `web/src/state/useCockpitData.ts`: bootstrap/detail state hooks.
- Create `web/src/components/*`: focused UI components.
- Create `web/src/styles.css`: global cockpit layout/style.
- Create `web/src/test/*`: test fixtures and helpers.
- Modify `docker-compose.dev2.yml`: add `web` service.
- Modify `.env.example`: document frontend API base/admin token behavior.
- Create `docs/dev-2-web-11-validation.md`: validation evidence.

## Task 1: Frontend Scaffold And API Contract Types

**Files:**
- Create: `web/package.json`
- Create: `web/vite.config.ts`
- Create: `web/tsconfig.json`
- Create: `web/tsconfig.node.json`
- Create: `web/index.html`
- Create: `web/src/api/types.ts`
- Create: `web/src/api/client.ts`
- Create: `web/src/test/fixtures.ts`
- Test: `web/src/api/client.test.ts`

- [ ] **Step 1: Write failing API client test**

Create `web/src/api/client.test.ts`:

```ts
import { afterEach, describe, expect, it, vi } from "vitest";
import { fetchBootstrap } from "./client";
import { bootstrapFixture } from "../test/fixtures";

afterEach(() => {
  vi.restoreAllMocks();
});

describe("cockpit api client", () => {
  it("fetches bootstrap from the configured api base", async () => {
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      json: async () => bootstrapFixture,
    });
    vi.stubGlobal("fetch", fetchMock);

    const data = await fetchBootstrap("http://backend.test/api");

    expect(fetchMock).toHaveBeenCalledWith("http://backend.test/api/cockpit/bootstrap");
    expect(data.markets.map((market) => market.summary.market_key)).toEqual(["btc5m", "eth15m"]);
  });

  it("throws a readable error for non-ok responses", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: false,
        status: 500,
        text: async () => "boom",
      }),
    );

    await expect(fetchBootstrap("/api")).rejects.toThrow("GET /cockpit/bootstrap failed: 500 boom");
  });
});
```

- [ ] **Step 2: Verify RED on dev-2**

Sync branch to a temporary dev-2 directory and run:

```bash
podman run --rm \
  -v "$PWD":/workspace -w /workspace/web \
  node:22-bookworm \
  sh -lc "npm install && npm test -- client.test.ts --runInBand"
```

Expected before implementation: FAIL because `web/package.json`, `client.ts`, and fixtures do not exist.

- [ ] **Step 3: Add scaffold and API client**

Create `web/package.json`:

```json
{
  "name": "polymarket-signal-cockpit-web",
  "private": true,
  "version": "0.1.0",
  "type": "module",
  "scripts": {
    "dev": "vite --host 0.0.0.0",
    "build": "tsc -b && vite build",
    "preview": "vite preview --host 0.0.0.0",
    "test": "vitest"
  },
  "dependencies": {
    "@vitejs/plugin-react": "^5.0.0",
    "lightweight-charts": "^5.0.0",
    "lucide-react": "^0.468.0",
    "react": "^19.0.0",
    "react-dom": "^19.0.0",
    "vite": "^6.0.0"
  },
  "devDependencies": {
    "@testing-library/jest-dom": "^6.6.0",
    "@testing-library/react": "^16.1.0",
    "@types/node": "^22.10.0",
    "@types/react": "^19.0.0",
    "@types/react-dom": "^19.0.0",
    "jsdom": "^25.0.0",
    "typescript": "^5.7.0",
    "vitest": "^2.1.0"
  }
}
```

Create contract types in `web/src/api/types.ts` that mirror `docs/web-cockpit-api-contract.md`, including `CockpitBootstrapResponse`, `CockpitMarket`, `MarketSummary`, `Signal`, `BacktestRun`, `NotificationChannel`, `NotificationDelivery`, `RuntimeHealth`, and `MarketsSnapshotMessage`.

Create `web/src/api/client.ts`:

```ts
import type {
  BacktestsResponse,
  CandlesResponse,
  CockpitBootstrapResponse,
  ModelAssignment,
  NotificationChannelsResponse,
  NotificationDeliveriesResponse,
  SignalsResponse,
} from "./types";

async function getJson<T>(apiBase: string, path: string): Promise<T> {
  const response = await fetch(`${apiBase.replace(/\/$/, "")}${path}`);
  if (!response.ok) {
    const body = await response.text();
    throw new Error(`GET ${path} failed: ${response.status} ${body}`);
  }
  return response.json() as Promise<T>;
}

export function fetchBootstrap(apiBase: string): Promise<CockpitBootstrapResponse> {
  return getJson(apiBase, "/cockpit/bootstrap");
}

export function fetchSignals(apiBase: string, marketKey: string, limit = 20): Promise<SignalsResponse> {
  return getJson(apiBase, `/signals?market_key=${encodeURIComponent(marketKey)}&limit=${limit}`);
}

export function fetchBacktests(apiBase: string, marketKey: string, modelKey?: string, limit = 20): Promise<BacktestsResponse> {
  const model = modelKey ? `&model_key=${encodeURIComponent(modelKey)}` : "";
  return getJson(apiBase, `/backtests?market_key=${encodeURIComponent(marketKey)}${model}&limit=${limit}`);
}

export function fetchCandles(apiBase: string, marketKey: string, limit = 120): Promise<CandlesResponse> {
  return getJson(apiBase, `/markets/${encodeURIComponent(marketKey)}/candles?limit=${limit}`);
}

export function fetchNotificationChannels(apiBase: string, marketKey: string): Promise<NotificationChannelsResponse> {
  return getJson(apiBase, `/config/notification-channels?market_key=${encodeURIComponent(marketKey)}`);
}

export function fetchNotificationDeliveries(apiBase: string, marketKey: string, limit = 20): Promise<NotificationDeliveriesResponse> {
  return getJson(apiBase, `/notifications/deliveries?market_key=${encodeURIComponent(marketKey)}&limit=${limit}`);
}
```

- [ ] **Step 4: Verify GREEN on dev-2**

Run:

```bash
podman run --rm \
  -v "$PWD":/workspace -w /workspace/web \
  node:22-bookworm \
  sh -lc "npm install && npm test -- client.test.ts"
```

Expected: API client tests pass.

- [ ] **Step 5: Commit**

```bash
git add web/package.json web/package-lock.json web/vite.config.ts web/tsconfig.json web/tsconfig.node.json web/index.html web/src/api web/src/test
git commit -m "feat: scaffold cockpit web api client"
```

## Task 2: Bootstrap State And WebSocket Reconnect

**Files:**
- Create: `web/src/api/ws.ts`
- Create: `web/src/state/useCockpitData.ts`
- Test: `web/src/state/useCockpitData.test.tsx`

- [ ] **Step 1: Write failing state test**

Test that:

- hook fetches bootstrap on mount.
- selected market defaults to `btc5m` if present.
- WS snapshot updates market summaries without deleting existing signals/backtests.
- WS error sets connection state to `reconnecting`.

- [ ] **Step 2: Verify RED on dev-2**

```bash
podman run --rm \
  -v "$PWD":/workspace -w /workspace/web \
  node:22-bookworm \
  sh -lc "npm install && npm test -- useCockpitData.test.tsx"
```

Expected: FAIL because hook and WS helper do not exist.

- [ ] **Step 3: Implement state hook**

Create state shape:

```ts
export type ConnectionState = "idle" | "connecting" | "live" | "reconnecting" | "failed";

export interface CockpitDataState {
  loading: boolean;
  error: string | null;
  connection: ConnectionState;
  selectedMarketKey: string | null;
  markets: CockpitMarket[];
  runtime: RuntimeHealth | null;
  generatedAt: string | null;
  selectMarket: (marketKey: string) => void;
  refresh: () => Promise<void>;
}
```

Implementation rules:

- fetch bootstrap first.
- derive selected market.
- open WS after bootstrap.
- merge WS `markets[]` into existing `CockpitMarket.summary`.
- reconnect with bounded backoff.
- refetch bootstrap after reconnect.

- [ ] **Step 4: Verify GREEN on dev-2**

Run the targeted hook test and then all web tests.

- [ ] **Step 5: Commit**

```bash
git add web/src/api/ws.ts web/src/state/useCockpitData.ts web/src/state/useCockpitData.test.tsx
git commit -m "feat: add cockpit bootstrap and websocket state"
```

## Task 3: Operator Layout And Market Cards

**Files:**
- Create: `web/src/App.tsx`
- Create: `web/src/components/RuntimeHeader.tsx`
- Create: `web/src/components/MarketCard.tsx`
- Create: `web/src/components/RecommendationPanel.tsx`
- Create: `web/src/components/StatusBadge.tsx`
- Create: `web/src/main.tsx`
- Create: `web/src/styles.css`
- Test: `web/src/components/MarketCard.test.tsx`
- Test: `web/src/App.test.tsx`

- [ ] **Step 1: Write failing component tests**

Tests:

- App renders BTC 5m and ETH 15m from bootstrap fixture.
- Market card shows active model and current recommendation.
- Live signal and actionable alert labels are both visible and distinct.
- No raw webhook URL appears in rendered output.
- Runtime header shows degraded status when runtime is missing or source stale.

- [ ] **Step 2: Verify RED on dev-2**

Run component tests in Node container. Expected: FAIL because components do not exist.

- [ ] **Step 3: Implement layout**

Design:

- Top `RuntimeHeader`.
- `.market-grid` with cards.
- `.workspace` with chart area and inspector.
- Use compact labels, monospaced numeric values, and explicit status colors.
- No hero, marketing copy, decorative orbs, or nested cards.

Recommendation priority:

1. actionable alert.
2. latest live signal.
3. no trade / no signal.

- [ ] **Step 4: Verify GREEN on dev-2**

Run targeted component tests and all web tests.

- [ ] **Step 5: Commit**

```bash
git add web/src/App.tsx web/src/main.tsx web/src/components web/src/styles.css
git commit -m "feat: render strategy cockpit shell"
```

## Task 4: Chart And Inspector Tabs

**Files:**
- Create: `web/src/components/ChartPanel.tsx`
- Create: `web/src/components/InspectorTabs.tsx`
- Create: `web/src/components/SignalsTab.tsx`
- Create: `web/src/components/BacktestTab.tsx`
- Test: `web/src/components/InspectorTabs.test.tsx`

- [ ] **Step 1: Write failing tests**

Tests:

- Signals tab lists latest signals newest-first.
- Backtest tab displays latest run metrics.
- Manual `Run Backtest` button is disabled with pending WEB-76 copy.
- Chart panel shows empty state when no candles exist.

- [ ] **Step 2: Verify RED on dev-2**

Run targeted tests. Expected: FAIL because components do not exist.

- [ ] **Step 3: Implement tabs and chart**

Chart rules:

- stable height on desktop and mobile.
- render candles using lightweight-charts.
- show empty state if no candles.
- markers for latest signal/actionable alert if enough timestamp data exists.

Backtest panel rules:

- show metrics defensively from JSON.
- render missing metrics as muted dash.
- disabled manual run button until WEB-76.

- [ ] **Step 4: Verify GREEN on dev-2**

Run targeted tests and all web tests.

- [ ] **Step 5: Commit**

```bash
git add web/src/components/ChartPanel.tsx web/src/components/InspectorTabs.tsx web/src/components/SignalsTab.tsx web/src/components/BacktestTab.tsx
git commit -m "feat: add cockpit chart and strategy inspector"
```

## Task 5: Strategy, Market, And Notification Controls

**Files:**
- Create: `web/src/components/StrategyConfigTab.tsx`
- Create: `web/src/components/MarketConfigTab.tsx`
- Create: `web/src/components/NotificationsTab.tsx`
- Modify: `web/src/api/client.ts`
- Test: `web/src/components/StrategyConfigTab.test.tsx`
- Test: `web/src/components/NotificationsTab.test.tsx`

- [ ] **Step 1: Write failing tests**

Tests:

- Strategy config validates JSON before save.
- Strategy config calls `PUT /api/config/model-assignments/{market_key}` with admin bearer token when provided.
- Strategy config shows auth error without clearing draft.
- Market config add/edit controls are disabled and reference pending WEB-75.
- Notification form saves webhook but renders only masked webhook after response.
- Dry-run result is displayed separately from real delivery history.

- [ ] **Step 2: Verify RED on dev-2**

Run targeted tests. Expected: FAIL because controls do not exist.

- [ ] **Step 3: Implement controls**

Rules:

- Admin token is read from in-memory UI state or env-derived value only.
- Do not persist token to localStorage.
- Do not log webhook URL.
- After notification save, clear raw webhook input.
- Render `webhook_url_masked`, never `webhook_url`.
- Disabled market config controls explain WEB-75 dependency.

- [ ] **Step 4: Verify GREEN on dev-2**

Run targeted tests and all web tests.

- [ ] **Step 5: Commit**

```bash
git add web/src/components/StrategyConfigTab.tsx web/src/components/MarketConfigTab.tsx web/src/components/NotificationsTab.tsx web/src/api/client.ts
git commit -m "feat: add strategy and notification controls"
```

## Task 6: dev-2 Web Service And Visual Verification

**Files:**
- Modify: `docker-compose.dev2.yml`
- Modify: `.env.example`
- Create: `docs/dev-2-web-11-validation.md`

- [ ] **Step 1: Add dev-2 web service**

Add service:

```yaml
  web:
    image: node:22-bookworm
    network_mode: host
    working_dir: /workspace/web
    command: sh -lc "npm install && npm run build && npm run preview -- --port ${DEV2_WEB_PORT:-5173}"
    environment:
      VITE_API_BASE: ${VITE_API_BASE:-http://192.168.103.157:8080/api}
      VITE_WS_BASE: ${VITE_WS_BASE:-ws://192.168.103.157:8080/api}
    volumes:
      - .:/workspace
      - ${HOME}/.cache/polymarket-node:/root/.npm
    depends_on:
      backend:
        condition: service_started
    restart: unless-stopped
```

- [ ] **Step 2: Run web tests/build on dev-2**

```bash
podman run --rm \
  -v "$PWD":/workspace -w /workspace/web \
  node:22-bookworm \
  sh -lc "npm install && npm test -- --run && npm run build"
```

Expected: tests and build pass.

- [ ] **Step 3: Start dev-2 stack**

```bash
DEV2_APP_PORT=8080 DEV2_WEB_PORT=5173 podman-compose -f docker-compose.dev2.yml up -d --force-recreate backend web
```

- [ ] **Step 4: Runtime smoke**

```bash
curl -fsS http://192.168.103.157:8080/healthz
curl -fsS http://192.168.103.157:5173
```

Expected: backend health ok and web HTML served.

- [ ] **Step 5: Browser verification**

Use Browser plugin against:

```text
http://192.168.103.157:5173
```

Verify:

- desktop screenshot has runtime header, BTC/ETH cards, recommendation panel, chart, inspector tabs.
- mobile screenshot has single market switcher, no overlapping text.
- chart area is nonblank or shows professional empty state.
- no raw webhook URL appears in visible DOM.

- [ ] **Step 6: Record validation**

Create `docs/dev-2-web-11-validation.md` with:

- branch and commit.
- Node container test/build evidence.
- dev-2 compose command.
- `/healthz` and web smoke.
- desktop/mobile visual findings.
- temporary container cleanup status.
- residual risks.

- [ ] **Step 7: Review, PR, Linear**

Request reviewer subagent. Fix Critical/Important findings. Push branch, create PR to `feature/rust-backend-baseline`, merge only after review approval and validation evidence.

## Self-Review Checklist

- [ ] The plan implements WEB-11 as a strategy operation cockpit, not a passive dashboard.
- [ ] Real-time recommendations are first-class and show side, price, size, confidence, TTL, reason, model, and features.
- [ ] Dynamic market config and manual backtest controls are visible but honest about pending WEB-75/WEB-76 APIs.
- [ ] Webhook and admin token handling avoid committed secrets and rendered raw webhook URLs.
- [ ] All tests and runtime validation happen on dev-2.
- [ ] No live trading, private keys, order execution, or geo-bypass behavior is introduced.

