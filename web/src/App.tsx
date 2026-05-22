import { MarketCard } from "./components/MarketCard";
import { RecommendationPanel } from "./components/RecommendationPanel";
import { RuntimeHeader } from "./components/RuntimeHeader";
import { useCockpitData } from "./state/useCockpitData";

export interface AppProps {
  apiBase?: string;
}

function configuredApiBase(): string {
  return import.meta.env.VITE_API_BASE ?? "/api";
}

export function App({ apiBase = configuredApiBase() }: AppProps) {
  const cockpit = useCockpitData({ apiBase });
  const selectedMarket =
    cockpit.markets.find((market) => market.summary.market_key === cockpit.selectedMarketKey) ?? cockpit.markets[0] ?? null;

  return (
    <main className="app-shell">
      <RuntimeHeader connection={cockpit.connection} generatedAt={cockpit.generatedAt} runtime={cockpit.runtime} />

      {cockpit.error ? <div className="error-banner">{cockpit.error}</div> : null}

      <section className="market-grid" aria-label="markets">
        {cockpit.markets.map((market) => (
          <MarketCard
            key={market.summary.market_key}
            market={market}
            selected={market.summary.market_key === selectedMarket?.summary.market_key}
            onSelect={() => cockpit.selectMarket(market.summary.market_key)}
          />
        ))}
      </section>

      <section className="workspace">
        <div className="workspace-main">
          <RecommendationPanel market={selectedMarket} />
          <section className="chart-placeholder">
            <div className="section-title-row">
              <h2>最近K线</h2>
              <span className="muted">{selectedMarket?.recent_candles.length ?? 0} candles</span>
            </div>
            <div className="chart-empty">Chart panel pending Task 4</div>
          </section>
        </div>
        <aside className="inspector-panel">
          <div className="section-title-row">
            <h2>策略证据</h2>
            <span className="muted">{selectedMarket?.active_model?.version ?? "no model"}</span>
          </div>
          <dl>
            <dt>回测</dt>
            <dd>{selectedMarket?.latest_backtest?.status ?? "pending"}</dd>
            <dt>通知</dt>
            <dd>{selectedMarket?.notification_deliveries[0]?.status ?? "none"}</dd>
            <dt>输入快照</dt>
            <dd>{selectedMarket?.latest_actionable_alert?.input_snapshot_hash ?? "-"}</dd>
          </dl>
        </aside>
      </section>
    </main>
  );
}
