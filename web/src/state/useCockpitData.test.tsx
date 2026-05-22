import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { bootstrapFixture } from "../test/fixtures";
import { useCockpitData } from "./useCockpitData";

class FakeWebSocket {
  static instances: FakeWebSocket[] = [];

  readonly url: string;
  onopen: ((event: Event) => void) | null = null;
  onmessage: ((event: MessageEvent) => void) | null = null;
  onerror: ((event: Event) => void) | null = null;
  onclose: ((event: CloseEvent) => void) | null = null;
  readyState = 0;

  constructor(url: string) {
    this.url = url;
    FakeWebSocket.instances.push(this);
  }

  close = vi.fn(() => {
    this.readyState = 3;
  });

  emitOpen() {
    this.readyState = 1;
    this.onopen?.(new Event("open"));
  }

  emitMessage(payload: unknown) {
    this.onmessage?.(new MessageEvent("message", { data: JSON.stringify(payload) }));
  }

  emitError() {
    this.onerror?.(new Event("error"));
  }

  emitClose() {
    this.readyState = 3;
    this.onclose?.(new CloseEvent("close"));
  }
}

beforeEach(() => {
  FakeWebSocket.instances = [];
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

describe("useCockpitData", () => {
  it("loads bootstrap, selects BTC by default, and merges websocket summaries", async () => {
    const { result } = renderHook(() => useCockpitData({ apiBase: "http://backend.test/api" }));

    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.selectedMarketKey).toBe("btc5m");
    expect(FakeWebSocket.instances[0]?.url).toBe("ws://backend.test/api/ws/markets");

    act(() => FakeWebSocket.instances[0].emitOpen());
    expect(result.current.connection).toBe("live");

    act(() =>
      FakeWebSocket.instances[0].emitMessage({
        type: "snapshot",
        generated_at: "2026-05-22T00:02:00Z",
        markets: [
          {
            ...bootstrapFixture.markets[0].summary,
            latest_tick: {
              ...bootstrapFixture.markets[0].summary.latest_tick!,
              price: "100222",
            },
          },
        ],
      }),
    );

    const btc = result.current.markets.find((market) => market.summary.market_key === "btc5m");
    expect(btc?.summary.latest_tick?.price).toBe("100222");
    expect(btc?.latest_actionable_alert?.id).toBe("00000000-0000-0000-0000-000000000004");
    await waitFor(() => expect(fetch).toHaveBeenCalledTimes(2));

    act(() => FakeWebSocket.instances[0].emitError());
    expect(result.current.connection).toBe("reconnecting");
  });

  it("refreshes full cockpit data after websocket snapshots so recommendations update", async () => {
    const refreshed = structuredClone(bootstrapFixture);
    refreshed.markets[0].latest_actionable_alert = {
      ...refreshed.markets[0].latest_actionable_alert!,
      id: "00000000-0000-0000-0000-000000000099",
      reason: "new realtime alert",
    };
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce({
        ok: true,
        json: async () => structuredClone(bootstrapFixture),
      })
      .mockResolvedValueOnce({
        ok: true,
        json: async () => refreshed,
      });
    vi.stubGlobal("fetch", fetchMock);

    const { result } = renderHook(() => useCockpitData({ apiBase: "http://backend.test/api" }));
    await waitFor(() => expect(result.current.loading).toBe(false));

    act(() =>
      FakeWebSocket.instances[0].emitMessage({
        type: "snapshot",
        generated_at: "2026-05-22T00:02:00Z",
        markets: [bootstrapFixture.markets[0].summary],
      }),
    );

    await waitFor(() =>
      expect(result.current.markets[0].latest_actionable_alert?.id).toBe("00000000-0000-0000-0000-000000000099"),
    );
    expect(result.current.markets[0].latest_actionable_alert?.reason).toBe("new realtime alert");
  });

  it("rate limits full bootstrap refreshes from frequent websocket snapshots", async () => {
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      json: async () => structuredClone(bootstrapFixture),
    });
    vi.stubGlobal("fetch", fetchMock);

    const { result } = renderHook(() => useCockpitData({ apiBase: "http://backend.test/api", fullRefreshMinMs: 1000 }));
    await waitFor(() => expect(result.current.loading).toBe(false));

    act(() =>
      FakeWebSocket.instances[0].emitMessage({
        type: "snapshot",
        generated_at: "2026-05-22T00:02:00Z",
        markets: [bootstrapFixture.markets[0].summary],
      }),
    );
    await waitFor(() => expect(fetchMock).toHaveBeenCalledTimes(2));

    act(() =>
      FakeWebSocket.instances[0].emitMessage({
        type: "snapshot",
        generated_at: "2026-05-22T00:02:01Z",
        markets: [bootstrapFixture.markets[0].summary],
      }),
    );

    await new Promise((resolve) => setTimeout(resolve, 30));
    expect(fetchMock).toHaveBeenCalledTimes(2);
  });

  it("ignores close events from replaced websocket instances", async () => {
    const { result } = renderHook(() =>
      useCockpitData({ apiBase: "http://backend.test/api", reconnectBaseMs: 10, reconnectMaxMs: 20 }),
    );
    await waitFor(() => expect(result.current.loading).toBe(false));
    const firstSocket = FakeWebSocket.instances[0];

    await act(async () => {
      await result.current.refresh();
    });
    expect(FakeWebSocket.instances).toHaveLength(2);

    await act(async () => {
      firstSocket.emitClose();
      await new Promise((resolve) => setTimeout(resolve, 30));
    });
    expect(FakeWebSocket.instances).toHaveLength(2);
  });
});
