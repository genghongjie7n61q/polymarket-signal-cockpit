import type { CockpitMarket } from "../api/types";

export interface MarketConfigTabProps {
  market: CockpitMarket;
}

export function MarketConfigTab({ market }: MarketConfigTabProps) {
  return (
    <div className="config-form">
      <label>
        品种 Key
        <input disabled value={market.summary.market_key} readOnly />
      </label>
      <label>
        交易对
        <input disabled value={market.summary.symbol} readOnly />
      </label>
      <button className="secondary-button" type="button" disabled>
        新增/编辑品种
      </button>
      <p className="muted">动态品种配置 API 等待 WEB-75。</p>
    </div>
  );
}
