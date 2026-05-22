export type JsonValue = null | boolean | number | string | JsonValue[] | { [key: string]: JsonValue };

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
  start_ts: string;
  end_ts: string;
  open_price?: string | null;
  latest_price?: string | null;
  direction?: string | null;
}

export interface MarketTick {
  symbol: string;
  source: string;
  source_ts: string;
  received_at: string;
  price: string;
  size: string | null;
  sequence: number | null;
}

export interface PolymarketSnapshot {
  event_slug: string;
  captured_at: string;
  up_price: string | null;
  down_price: string | null;
  spread: string | null;
  liquidity: string | null;
}

export interface Candle {
  start_ts: string;
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
  created_at: string;
}

export interface ModelAssignment {
  market_key: string;
  model_key: string;
  display_name: string;
  version: string;
  parameters: JsonValue;
  status: string;
  created_at: string;
}

export interface BacktestRun {
  id: string;
  market_key: string;
  model_key: string;
  display_name: string;
  model_version: string;
  parameters: JsonValue;
  started_at: string;
  finished_at: string | null;
  window_start: string;
  window_end: string;
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
  created_at: string;
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
  created_at: string;
  updated_at: string;
}

export interface RuntimeHealth {
  storage_writer: Record<string, JsonValue> | null;
  realtime: Record<string, JsonValue> | null;
  notification: Record<string, JsonValue> | null;
}

export interface MarketsSnapshotMessage {
  type: "snapshot";
  generated_at: string;
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
