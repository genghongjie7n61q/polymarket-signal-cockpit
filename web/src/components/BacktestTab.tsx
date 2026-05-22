import type { BacktestRun, JsonValue } from "../api/types";

export interface BacktestTabProps {
  backtest: BacktestRun | null;
}

function isMetricObject(value: JsonValue): value is Record<string, JsonValue> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function displayMetric(value: JsonValue): string {
  if (value === null) {
    return "-";
  }
  if (typeof value === "object") {
    return JSON.stringify(value);
  }
  return String(value);
}

export function BacktestTab({ backtest }: BacktestTabProps) {
  const metrics = backtest && isMetricObject(backtest.metrics) ? Object.entries(backtest.metrics) : [];

  return (
    <div className="backtest-tab">
      <button className="secondary-button" type="button" disabled>
        Run Backtest
      </button>
      <p className="muted">手动回测任务 API 等待 WEB-76。</p>
      {backtest ? (
        <dl className="metrics-grid">
          <dt>status</dt>
          <dd>{backtest.status}</dd>
          {metrics.map(([key, value]) => (
            <div className="metric-pair" key={key}>
              <dt>{key}</dt>
              <dd>{displayMetric(value)}</dd>
            </div>
          ))}
        </dl>
      ) : (
        <p className="muted">暂无回测结果</p>
      )}
    </div>
  );
}
