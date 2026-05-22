import type {
  BacktestsResponse,
  CandlesResponse,
  CockpitBootstrapResponse,
  FeishuDryRunRequest,
  FeishuDryRunResponse,
  ModelAssignment,
  NotificationChannelsResponse,
  NotificationChannel,
  NotificationDeliveriesResponse,
  SignalsResponse,
  UpsertNotificationChannelRequest,
} from "./types";

function normalizeApiBase(apiBase: string): string {
  return apiBase.replace(/\/$/, "");
}

async function getJson<T>(apiBase: string, path: string): Promise<T> {
  const response = await fetch(`${normalizeApiBase(apiBase)}${path}`);
  if (!response.ok) {
    const body = await response.text();
    throw new Error(`GET ${path} failed: ${response.status} ${body}`);
  }
  return response.json() as Promise<T>;
}

async function sendJson<T>(apiBase: string, path: string, method: string, body: unknown, adminToken?: string): Promise<T> {
  const headers: Record<string, string> = { "content-type": "application/json" };
  if (adminToken) {
    headers.authorization = `Bearer ${adminToken}`;
  }

  const response = await fetch(`${normalizeApiBase(apiBase)}${path}`, {
    method,
    headers,
    body: JSON.stringify(body),
  });
  if (!response.ok) {
    const text = await response.text();
    throw new Error(`${method} ${path} failed: ${response.status} ${text}`);
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

export function saveModelAssignment(
  apiBase: string,
  marketKey: string,
  assignment: Pick<ModelAssignment, "model_key" | "display_name" | "version" | "parameters">,
  adminToken?: string,
): Promise<ModelAssignment> {
  return sendJson(apiBase, `/config/model-assignments/${encodeURIComponent(marketKey)}`, "PUT", assignment, adminToken);
}

export function saveNotificationChannel(
  apiBase: string,
  request: UpsertNotificationChannelRequest,
  adminToken?: string,
): Promise<NotificationChannel> {
  return sendJson(apiBase, "/config/notification-channels", "POST", request, adminToken);
}

export function sendFeishuDryRun(
  apiBase: string,
  request: FeishuDryRunRequest,
  adminToken?: string,
): Promise<FeishuDryRunResponse> {
  return sendJson(apiBase, "/notifications/feishu/dry-run", "POST", request, adminToken);
}
