import type { CockpitMarket, Signal } from "../api/types";

export interface SignalsTabProps {
  market: CockpitMarket;
}

function signalsFor(market: CockpitMarket): Array<{ label: string; signal: Signal }> {
  return [
    market.latest_actionable_alert ? { label: "Actionable Alert", signal: market.latest_actionable_alert } : null,
    market.latest_signal ? { label: "Live Signal", signal: market.latest_signal } : null,
  ]
    .filter((item): item is { label: string; signal: Signal } => item !== null)
    .sort((left, right) => Date.parse(right.signal.created_at) - Date.parse(left.signal.created_at));
}

export function SignalsTab({ market }: SignalsTabProps) {
  const signals = signalsFor(market);

  if (signals.length === 0) {
    return <p className="muted">暂无信号</p>;
  }

  return (
    <div className="signal-list">
      {signals.map(({ label, signal }) => (
        <div className="signal-row" data-testid="signal-row" key={signal.id}>
          <span>{label}</span>
          <strong>{signal.side ?? "-"}</strong>
          <span>{signal.limit_price ?? "-"}</span>
          <span>{signal.created_at}</span>
        </div>
      ))}
    </div>
  );
}
