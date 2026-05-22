import type { DateTimeValue } from "./time";

export type JsonValue = null | boolean | number | string | JsonValue[] | { [key: string]: JsonValue };

export interface CockpitBootstrapResponse {
  generated_at: DateTimeValue;
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

export interface MarketSummary {
  market_key: string;
  symbol: string;
  interval_seconds: number;
  source_status: string | null;
  current_window: MarketWindow | null;
  latest_tick: MarketTick | null;
  latest_snapshot: PolymarketSnapshot | null;
}

export interface MarketWindow {
  event_slug: string;
  start_ts: DateTimeValue;
  end_ts: DateTimeValue;
  open_price?: string | null;
  latest_price?: string | null;
  direction?: string | null;
}

export interface MarketTick {
  symbol: string;
  source: string;
  source_ts: DateTimeValue;
  received_at: DateTimeValue;
  price: string;
  size: string | null;
  sequence: number | null;
}

export interface PolymarketSnapshot {
  event_slug: string;
  captured_at: DateTimeValue;
  up_price: string | null;
  down_price: string | null;
  spread: string | null;
  liquidity: string | null;
}

export interface Candle {
  start_ts: DateTimeValue;
  open: string;
  high: string;
  low: string;
  close: string;
  volume: string;
}

export interface Signal {
  id: string;
  market_window_id: string;
  model_version_id: string;
  signal_type: string;
  side: string | null;
  confidence: string | null;
  limit_price: string | null;
  suggested_size: string | null;
  ttl_ms: number | null;
  reason: string;
  features: JsonValue;
  input_snapshot_hash: string;
  created_at: DateTimeValue;
}

export interface ModelAssignment {
  market_key: string;
  model_key: string;
  display_name: string;
  version: string;
  parameters: JsonValue;
  status: string;
  created_at: DateTimeValue;
}

export interface BacktestRun {
  id: string;
  market_key: string;
  model_key: string;
  display_name: string;
  model_version: string;
  parameters: JsonValue;
  started_at: DateTimeValue;
  finished_at: DateTimeValue | null;
  window_start: DateTimeValue;
  window_end: DateTimeValue;
  metrics: JsonValue;
  status: string;
}

export interface NotificationChannel {
  id: string;
  market_key: string;
  channel_type: string;
  name: string;
  webhook_url: null;
  webhook_url_masked: string;
  enabled: boolean;
  created_at: DateTimeValue;
}

export interface NotificationDelivery {
  id: string;
  market_key: string;
  signal_id: string;
  channel_id: string;
  channel_type: string;
  channel_name: string;
  status: string;
  attempt_count: number;
  response_summary: string | null;
  created_at: DateTimeValue;
  updated_at: DateTimeValue;
}

export interface RuntimeHealth {
  storage_writer: Record<string, JsonValue> | null;
  realtime: Record<string, JsonValue> | null;
  notification: Record<string, JsonValue> | null;
}

export interface MarketsSnapshotMessage {
  type: "snapshot";
  generated_at: DateTimeValue;
  markets: MarketSummary[];
}

export interface SignalsResponse {
  market_key: string;
  signals: Signal[];
}

export interface BacktestsResponse {
  runs: BacktestRun[];
}

export interface CandlesResponse {
  market_key: string;
  candles: Candle[];
}

export interface NotificationChannelsResponse {
  market_key: string;
  channels: NotificationChannel[];
}

export interface NotificationDeliveriesResponse {
  market_key: string;
  deliveries: NotificationDelivery[];
}

export interface UpsertNotificationChannelRequest {
  market_key: string;
  channel_type: "feishu";
  name: string;
  webhook_url: string;
  enabled: boolean;
}

export interface FeishuDryRunRequest {
  market_key: string;
  channel_name?: string;
}

export interface FeishuDryRunResponse {
  market_key: string;
  card_summary: {
    title: string;
    reason: string;
  };
  sent: Array<{
    channel: NotificationChannel;
    status: string;
    response_summary: string | null;
  }>;
  notification: Record<string, JsonValue> | null;
}
