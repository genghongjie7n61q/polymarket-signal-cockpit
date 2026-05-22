import { fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { bootstrapFixture } from "../test/fixtures";
import { ChartPanel } from "./ChartPanel";
import { InspectorTabs } from "./InspectorTabs";

describe("InspectorTabs", () => {
  it("lists signals newest first", () => {
    render(<InspectorTabs market={bootstrapFixture.markets[0]} />);

    const rows = screen.getAllByTestId("signal-row");
    expect(within(rows[0]).getByText("Actionable Alert")).toBeInTheDocument();
    expect(within(rows[1]).getByText("Live Signal")).toBeInTheDocument();
  });

  it("displays backtest metrics and keeps manual run disabled until WEB-76", () => {
    render(<InspectorTabs market={bootstrapFixture.markets[0]} />);

    fireEvent.click(screen.getByRole("tab", { name: "回测" }));
    expect(screen.getByText("trades")).toBeInTheDocument();
    expect(screen.getByText("40")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Run Backtest" })).toBeDisabled();
    expect(screen.getByText(/WEB-76/)).toBeInTheDocument();
  });

  it("renders chart empty state when a market has no candles", () => {
    render(<ChartPanel market={bootstrapFixture.markets[1]} />);

    expect(screen.getByText("暂无K线数据")).toBeInTheDocument();
  });
});
