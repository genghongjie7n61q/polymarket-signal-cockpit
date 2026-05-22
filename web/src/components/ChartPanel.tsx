import { useEffect, useRef } from "react";
import { CandlestickSeries, createChart, type IChartApi, type UTCTimestamp } from "lightweight-charts";
import { dateTimeToMillis } from "../api/time";
import type { Candle, CockpitMarket } from "../api/types";

export interface ChartPanelProps {
  market: CockpitMarket | null;
}

function toSeriesData(candles: Candle[]) {
  return candles.map((candle) => ({
    time: Math.floor((dateTimeToMillis(candle.start_ts) ?? 0) / 1000) as UTCTimestamp,
    open: Number(candle.open),
    high: Number(candle.high),
    low: Number(candle.low),
    close: Number(candle.close),
  }));
}

function canRenderCanvasChart(): boolean {
  if (typeof window.matchMedia !== "function") {
    return false;
  }
  try {
    return Boolean(document.createElement("canvas").getContext("2d"));
  } catch {
    return false;
  }
}

export function ChartPanel({ market }: ChartPanelProps) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const chartRef = useRef<IChartApi | null>(null);
  const candles = market?.recent_candles ?? [];

  useEffect(() => {
    if (!containerRef.current || candles.length === 0 || !canRenderCanvasChart()) {
      return undefined;
    }

    const chart = createChart(containerRef.current, {
      height: 300,
      layout: { background: { color: "#ffffff" }, textColor: "#44515d" },
      grid: { horzLines: { color: "#edf0f2" }, vertLines: { color: "#edf0f2" } },
      rightPriceScale: { borderColor: "#d7dde3" },
      timeScale: { borderColor: "#d7dde3" },
    });
    const series = chart.addSeries(CandlestickSeries, {
      upColor: "#15864b",
      downColor: "#c7372f",
      borderVisible: false,
      wickUpColor: "#15864b",
      wickDownColor: "#c7372f",
    });
    series.setData(toSeriesData(candles));
    chart.timeScale().fitContent();
    chartRef.current = chart;

    return () => {
      chart.remove();
      chartRef.current = null;
    };
  }, [candles]);

  return (
    <section className="chart-placeholder">
      <div className="section-title-row">
        <h2>最近K线</h2>
        <span className="muted">{candles.length} candles</span>
      </div>
      {candles.length > 0 ? (
        <div ref={containerRef} className="chart-surface" aria-label={`${market?.summary.symbol ?? "market"} candles`} />
      ) : (
        <div className="chart-empty">暂无K线数据</div>
      )}
    </section>
  );
}
