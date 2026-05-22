import { useCallback, useEffect, useRef, useState } from "react";
import { fetchBootstrap } from "../api/client";
import { openMarketsWebSocket, type MarketWebSocketHandle } from "../api/ws";
import type { CockpitMarket, MarketSummary, RuntimeHealth } from "../api/types";
import type { DateTimeValue } from "../api/time";

export type ConnectionState = "idle" | "connecting" | "live" | "reconnecting" | "failed";

export interface CockpitDataState {
  loading: boolean;
  error: string | null;
  connection: ConnectionState;
  selectedMarketKey: string | null;
  markets: CockpitMarket[];
  runtime: RuntimeHealth | null;
  generatedAt: DateTimeValue | null;
  selectMarket: (marketKey: string) => void;
  refresh: () => Promise<void>;
}

export interface UseCockpitDataOptions {
  apiBase: string;
  reconnectBaseMs?: number;
  reconnectMaxMs?: number;
}

function defaultMarketKey(markets: CockpitMarket[]): string | null {
  return (
    markets.find((market) => market.summary.market_key === "btc5m")?.summary.market_key ??
    markets[0]?.summary.market_key ??
    null
  );
}

function emptyMarket(summary: MarketSummary): CockpitMarket {
  return {
    summary,
    recent_candles: [],
    active_model: null,
    latest_signal: null,
    latest_actionable_alert: null,
    latest_backtest: null,
    notification_channels: [],
    notification_deliveries: [],
  };
}

function mergeMarketSummaries(markets: CockpitMarket[], summaries: MarketSummary[]): CockpitMarket[] {
  const byKey = new Map(summaries.map((summary) => [summary.market_key, summary]));
  const seen = new Set<string>();
  const merged = markets.map((market) => {
    const summary = byKey.get(market.summary.market_key);
    if (!summary) {
      return market;
    }
    seen.add(summary.market_key);
    return { ...market, summary };
  });

  for (const summary of summaries) {
    if (!seen.has(summary.market_key)) {
      merged.push(emptyMarket(summary));
    }
  }

  return merged;
}

export function useCockpitData({
  apiBase,
  reconnectBaseMs = 500,
  reconnectMaxMs = 5000,
}: UseCockpitDataOptions): CockpitDataState {
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [connection, setConnection] = useState<ConnectionState>("idle");
  const [selectedMarketKey, setSelectedMarketKey] = useState<string | null>(null);
  const [markets, setMarkets] = useState<CockpitMarket[]>([]);
  const [runtime, setRuntime] = useState<RuntimeHealth | null>(null);
  const [generatedAt, setGeneratedAt] = useState<DateTimeValue | null>(null);
  const wsRef = useRef<MarketWebSocketHandle | null>(null);
  const reconnectTimerRef = useRef<number | null>(null);
  const reconnectAttemptRef = useRef(0);
  const disposedRef = useRef(false);

  const clearReconnectTimer = useCallback(() => {
    if (reconnectTimerRef.current !== null) {
      window.clearTimeout(reconnectTimerRef.current);
      reconnectTimerRef.current = null;
    }
  }, []);

  const connectWebSocket = useCallback(() => {
    clearReconnectTimer();
    wsRef.current?.close();
    setConnection((current) => (current === "reconnecting" ? "reconnecting" : "connecting"));

    wsRef.current = openMarketsWebSocket(apiBase, {
      onOpen: () => {
        reconnectAttemptRef.current = 0;
        setConnection("live");
      },
      onSnapshot: (message) => {
        setGeneratedAt(message.generated_at);
        setMarkets((current) => mergeMarketSummaries(current, message.markets));
      },
      onError: () => {
        setConnection("reconnecting");
      },
      onClose: () => {
        if (disposedRef.current) {
          return;
        }
        setConnection("reconnecting");
        const attempt = reconnectAttemptRef.current + 1;
        reconnectAttemptRef.current = attempt;
        const delay = Math.min(reconnectBaseMs * 2 ** (attempt - 1), reconnectMaxMs);
        reconnectTimerRef.current = window.setTimeout(connectWebSocket, delay);
      },
    });
  }, [apiBase, clearReconnectTimer, reconnectBaseMs, reconnectMaxMs]);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const bootstrap = await fetchBootstrap(apiBase);
      setMarkets(bootstrap.markets);
      setRuntime(bootstrap.runtime);
      setGeneratedAt(bootstrap.generated_at);
      setSelectedMarketKey((current) =>
        current && bootstrap.markets.some((market) => market.summary.market_key === current)
          ? current
          : defaultMarketKey(bootstrap.markets),
      );
      connectWebSocket();
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : String(caught));
      setConnection("failed");
    } finally {
      setLoading(false);
    }
  }, [apiBase, connectWebSocket]);

  useEffect(() => {
    disposedRef.current = false;
    void refresh();
    return () => {
      disposedRef.current = true;
      clearReconnectTimer();
      wsRef.current?.close();
      wsRef.current = null;
    };
  }, [clearReconnectTimer, refresh]);

  return {
    loading,
    error,
    connection,
    selectedMarketKey,
    markets,
    runtime,
    generatedAt,
    selectMarket: setSelectedMarketKey,
    refresh,
  };
}
