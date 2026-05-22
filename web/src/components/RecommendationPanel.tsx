import type { CockpitMarket, Signal } from "../api/types";
import { StatusBadge } from "./StatusBadge";

export interface RecommendationPanelProps {
  market: CockpitMarket | null;
}

function chosenSignal(market: CockpitMarket): { label: string; signal: Signal | null } {
  if (market.latest_actionable_alert) {
    return { label: "Actionable Alert", signal: market.latest_actionable_alert };
  }
  if (market.latest_signal) {
    return { label: "Live Signal", signal: market.latest_signal };
  }
  return { label: "No Trade", signal: null };
}

export function RecommendationPanel({ market }: RecommendationPanelProps) {
  if (!market) {
    return (
      <section className="recommendation-panel">
        <h2>当前下单建议</h2>
        <p className="muted">等待市场数据</p>
      </section>
    );
  }

  const recommendation = chosenSignal(market);
  const signal = recommendation.signal;

  return (
    <section className="recommendation-panel">
      <div className="section-title-row">
        <h2>当前下单建议</h2>
        <StatusBadge label={recommendation.label} tone={signal ? "good" : "neutral"} />
      </div>
      {signal ? (
        <div className="recommendation-grid">
          <div>
            <span>方向</span>
            <strong>{signal.side ?? "-"}</strong>
          </div>
          <div>
            <span>限价</span>
            <strong>{signal.limit_price ?? "-"}</strong>
          </div>
          <div>
            <span>建议仓位</span>
            <strong>{signal.suggested_size ?? "-"}</strong>
          </div>
          <div>
            <span>置信度</span>
            <strong>{signal.confidence ? `${Math.round(Number(signal.confidence) * 100)}%` : "-"}</strong>
          </div>
          <div>
            <span>TTL</span>
            <strong>{signal.ttl_ms ? `${Math.round(signal.ttl_ms / 1000)}s` : "-"}</strong>
          </div>
          <div>
            <span>模型</span>
            <strong>{market.active_model?.display_name ?? "-"}</strong>
          </div>
        </div>
      ) : (
        <p className="muted">当前没有可执行信号</p>
      )}
      {signal ? <p className="recommendation-reason">{signal.reason}</p> : null}
    </section>
  );
}
