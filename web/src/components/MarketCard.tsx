import type { CockpitMarket, Signal } from "../api/types";
import { StatusBadge } from "./StatusBadge";

export interface MarketCardProps {
  market: CockpitMarket;
  selected: boolean;
  onSelect: () => void;
}

function intervalLabel(seconds: number): string {
  if (seconds % 60 === 0) {
    return `${seconds / 60}m`;
  }
  return `${seconds}s`;
}

function price(value: string | null | undefined): string {
  return value ?? "-";
}

function SignalLine({ label, signal }: { label: string; signal: Signal | null }) {
  return (
    <div className="signal-line">
      <span className="signal-line__label">{label}</span>
      {signal ? (
        <>
          <strong>{signal.side ?? "No side"}</strong>
          <span>{signal.limit_price ? `@ ${signal.limit_price}` : "no limit"}</span>
          <span>{signal.confidence ? `${Math.round(Number(signal.confidence) * 100)}%` : "-"}</span>
        </>
      ) : (
        <span className="muted">none</span>
      )}
    </div>
  );
}

export function MarketCard({ market, selected, onSelect }: MarketCardProps) {
  const { summary } = market;
  const modelLabel = market.active_model?.display_name ?? "未选择模型";
  const channelCount = market.notification_channels.length;

  return (
    <button className={`market-card ${selected ? "market-card--selected" : ""}`} type="button" onClick={onSelect}>
      <div className="market-card__top">
        <div>
          <span className="market-card__symbol">{summary.symbol}</span>
          <span className="market-card__interval">{intervalLabel(summary.interval_seconds)}</span>
        </div>
        <StatusBadge label={summary.source_status ?? "unknown"} tone={summary.source_status === "fresh" ? "good" : "warn"} />
      </div>
      <div className="market-card__price">
        <span>现价</span>
        <strong>{price(summary.latest_tick?.price ?? summary.current_window?.latest_price)}</strong>
      </div>
      <div className="market-card__meta">
        <span>模型</span>
        <strong>{modelLabel}</strong>
      </div>
      <SignalLine label="Actionable Alert" signal={market.latest_actionable_alert} />
      <SignalLine label="Live Signal" signal={market.latest_signal} />
      <div className="market-card__footer">
        <span>{summary.current_window?.event_slug ?? "no active window"}</span>
        <span>{channelCount} webhook</span>
      </div>
    </button>
  );
}
