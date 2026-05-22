import type { MarketsSnapshotMessage } from "./types";

export interface MarketWebSocketCallbacks {
  onOpen?: () => void;
  onSnapshot: (message: MarketsSnapshotMessage) => void;
  onError?: (event: Event) => void;
  onClose?: (event: CloseEvent) => void;
}

export interface MarketWebSocketHandle {
  readonly url: string;
  close: () => void;
}

function normalizeApiBase(apiBase: string): string {
  return apiBase.replace(/\/$/, "");
}

export function marketsWebSocketUrl(apiBase: string): string {
  const baseUrl = new URL(`${normalizeApiBase(apiBase)}/ws/markets`, window.location.origin);
  baseUrl.protocol = baseUrl.protocol === "https:" ? "wss:" : "ws:";
  return baseUrl.toString();
}

export function openMarketsWebSocket(apiBase: string, callbacks: MarketWebSocketCallbacks): MarketWebSocketHandle {
  const url = marketsWebSocketUrl(apiBase);
  const socket = new WebSocket(url);

  socket.onopen = () => callbacks.onOpen?.();
  socket.onmessage = (event) => {
    const message = JSON.parse(event.data as string) as MarketsSnapshotMessage;
    if (message.type === "snapshot") {
      callbacks.onSnapshot(message);
    }
  };
  socket.onerror = (event) => callbacks.onError?.(event);
  socket.onclose = (event) => callbacks.onClose?.(event);

  return {
    url,
    close: () => socket.close(),
  };
}
