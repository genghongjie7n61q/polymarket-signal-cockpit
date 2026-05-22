import { ChartPanel } from "./components/ChartPanel";
import { InspectorTabs } from "./components/InspectorTabs";
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
          <ChartPanel market={selectedMarket} />
        </div>
        <InspectorTabs market={selectedMarket} />
      </section>
    </main>
  );
}
