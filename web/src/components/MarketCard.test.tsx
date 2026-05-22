import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { bootstrapFixture } from "../test/fixtures";
import { MarketCard } from "./MarketCard";

describe("MarketCard", () => {
  it("shows active model and distinct recommendation labels", () => {
    render(<MarketCard market={bootstrapFixture.markets[0]} selected={true} onSelect={() => undefined} />);

    expect(screen.getByText("BTC-USD")).toBeInTheDocument();
    expect(screen.getByText("Baseline Direction")).toBeInTheDocument();
    expect(screen.getByText("Actionable Alert")).toBeInTheDocument();
    expect(screen.getByText("Live Signal")).toBeInTheDocument();
    expect(screen.getAllByText("Up").length).toBeGreaterThan(0);
  });

  it("never renders a raw Feishu webhook path", () => {
    render(<MarketCard market={bootstrapFixture.markets[0]} selected={false} onSelect={() => undefined} />);

    expect(screen.queryByText(/open-apis\/bot\/v2\/hook/i)).not.toBeInTheDocument();
  });
});
