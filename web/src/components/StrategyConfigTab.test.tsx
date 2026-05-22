import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { bootstrapFixture } from "../test/fixtures";
import { StrategyConfigTab } from "./StrategyConfigTab";

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe("StrategyConfigTab", () => {
  it("validates JSON parameters before save", () => {
    render(<StrategyConfigTab adminToken="token" apiBase="/api" market={bootstrapFixture.markets[0]} />);

    fireEvent.change(screen.getByLabelText("参数 JSON"), { target: { value: "{bad" } });
    fireEvent.click(screen.getByRole("button", { name: "保存模型" }));

    expect(screen.getByText("参数不是有效 JSON")).toBeInTheDocument();
  });

  it("saves model assignment with bearer token", async () => {
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      json: async () => bootstrapFixture.markets[0].active_model,
    });
    vi.stubGlobal("fetch", fetchMock);
    render(<StrategyConfigTab adminToken="admin-token" apiBase="http://backend.test/api" market={bootstrapFixture.markets[0]} />);

    fireEvent.click(screen.getByRole("button", { name: "保存模型" }));

    await waitFor(() => expect(screen.getByText("模型配置已保存")).toBeInTheDocument());
    expect(fetchMock).toHaveBeenCalledWith(
      "http://backend.test/api/config/model-assignments/btc5m",
      expect.objectContaining({
        method: "PUT",
        headers: expect.objectContaining({ authorization: "Bearer admin-token" }),
      }),
    );
  });

  it("shows auth error without clearing draft", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: false,
        status: 401,
        text: async () => "unauthorized",
      }),
    );
    render(<StrategyConfigTab adminToken="" apiBase="/api" market={bootstrapFixture.markets[0]} />);

    fireEvent.change(screen.getByLabelText("模型 Key"), { target: { value: "mean-reversion" } });
    fireEvent.click(screen.getByRole("button", { name: "保存模型" }));

    await waitFor(() => expect(screen.getByText(/401 unauthorized/)).toBeInTheDocument());
    expect(screen.getByDisplayValue("mean-reversion")).toBeInTheDocument();
  });
});
