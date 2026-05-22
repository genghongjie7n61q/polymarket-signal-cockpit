import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { bootstrapFixture } from "../test/fixtures";
import { MarketConfigTab } from "./MarketConfigTab";

describe("MarketConfigTab", () => {
  it("keeps dynamic market edits disabled until WEB-75", () => {
    render(<MarketConfigTab market={bootstrapFixture.markets[0]} />);

    expect(screen.getByRole("button", { name: "新增/编辑品种" })).toBeDisabled();
    expect(screen.getByText(/WEB-75/)).toBeInTheDocument();
  });
});
