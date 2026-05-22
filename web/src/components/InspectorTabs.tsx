import { useState } from "react";
import type { CockpitMarket } from "../api/types";
import { BacktestTab } from "./BacktestTab";
import { MarketConfigTab } from "./MarketConfigTab";
import { NotificationsTab } from "./NotificationsTab";
import { SignalsTab } from "./SignalsTab";
import { StrategyConfigTab } from "./StrategyConfigTab";

export interface InspectorTabsProps {
  market: CockpitMarket | null;
  apiBase: string;
  adminToken: string;
}

type TabKey = "signals" | "backtest" | "strategy" | "market" | "notifications";

export function InspectorTabs({ market, apiBase, adminToken }: InspectorTabsProps) {
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
        <button aria-selected={tab === "strategy"} role="tab" type="button" onClick={() => setTab("strategy")}>
          策略
        </button>
        <button aria-selected={tab === "market"} role="tab" type="button" onClick={() => setTab("market")}>
          品种
        </button>
        <button aria-selected={tab === "notifications"} role="tab" type="button" onClick={() => setTab("notifications")}>
          通知
        </button>
      </div>
      <div className="tab-panel">
        {!market ? <p className="muted">等待市场选择</p> : null}
        {market && tab === "signals" ? <SignalsTab market={market} /> : null}
        {market && tab === "backtest" ? <BacktestTab backtest={market.latest_backtest} /> : null}
        {market && tab === "strategy" ? <StrategyConfigTab adminToken={adminToken} apiBase={apiBase} market={market} /> : null}
        {market && tab === "market" ? <MarketConfigTab market={market} /> : null}
        {market && tab === "notifications" ? <NotificationsTab adminToken={adminToken} apiBase={apiBase} market={market} /> : null}
      </div>
    </aside>
  );
}
