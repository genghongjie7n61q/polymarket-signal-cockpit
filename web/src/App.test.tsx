import { render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { bootstrapFixture } from "./test/fixtures";
import { App } from "./App";

class FakeWebSocket {
  onopen: (() => void) | null = null;
  onmessage: ((event: MessageEvent) => void) | null = null;
  onerror: ((event: Event) => void) | null = null;
  onclose: ((event: CloseEvent) => void) | null = null;

  constructor(readonly url: string) {}

  close() {}
}

beforeEach(() => {
  vi.stubGlobal("WebSocket", FakeWebSocket);
  vi.stubGlobal(
    "fetch",
    vi.fn().mockResolvedValue({
      ok: true,
      json: async () => structuredClone(bootstrapFixture),
    }),
  );
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe("App", () => {
  it("renders the operator cockpit from bootstrap data", async () => {
    render(<App apiBase="http://backend.test/api" />);

    await waitFor(() => expect(screen.getByText("BTC-USD")).toBeInTheDocument());
    expect(screen.getByText("ETH-USD")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "当前下单建议" })).toBeInTheDocument();
    expect(screen.getAllByText("Baseline Direction").length).toBeGreaterThan(0);
    expect(screen.queryByText(/open-apis\/bot\/v2\/hook/i)).not.toBeInTheDocument();
  });
});
