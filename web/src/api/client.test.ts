import { afterEach, describe, expect, it, vi } from "vitest";
import { fetchBootstrap } from "./client";
import { bootstrapFixture } from "../test/fixtures";

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe("cockpit api client", () => {
  it("fetches bootstrap from the configured api base", async () => {
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      json: async () => bootstrapFixture,
    });
    vi.stubGlobal("fetch", fetchMock);

    const data = await fetchBootstrap("http://backend.test/api");

    expect(fetchMock).toHaveBeenCalledWith("http://backend.test/api/cockpit/bootstrap");
    expect(data.markets.map((market) => market.summary.market_key)).toEqual(["btc5m", "eth15m"]);
  });

  it("throws a readable error for non-ok responses", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: false,
        status: 500,
        text: async () => "boom",
      }),
    );

    await expect(fetchBootstrap("/api")).rejects.toThrow("GET /cockpit/bootstrap failed: 500 boom");
  });
});
