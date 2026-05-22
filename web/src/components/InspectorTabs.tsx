import { useState } from "react";
import type { CockpitMarket } from "../api/types";
import { BacktestTab } from "./BacktestTab";
import { SignalsTab } from "./SignalsTab";

export interface InspectorTabsProps {
  market: CockpitMarket | null;
}

type TabKey = "signals" | "backtest";

export function InspectorTabs({ market }: InspectorTabsProps) {
  const [tab, setTab] = useState<TabKey>("signals");

  return (
    <aside className="inspector-panel">
      <div className="section-title-row">
        <h2>策略证据</h2>
        <span className="muted">{market?.active_model?.version ?? "no model"}</span>
      </div>
      <div className="tab-list" role="tablist" aria-label="strategy inspector">
        <button aria-selected={tab === "signals"} role="tab" type="button" onClick={() => setTab("signals")}>
          信号
        </button>
        <button aria-selected={tab === "backtest"} role="tab" type="button" onClick={() => setTab("backtest")}>
          回测
        </button>
      </div>
      <div className="tab-panel">
        {!market ? <p className="muted">等待市场选择</p> : null}
        {market && tab === "signals" ? <SignalsTab market={market} /> : null}
        {market && tab === "backtest" ? <BacktestTab backtest={market.latest_backtest} /> : null}
      </div>
    </aside>
  );
}
