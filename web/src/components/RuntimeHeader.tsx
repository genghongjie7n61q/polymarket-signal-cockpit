import type { RuntimeHealth } from "../api/types";
import { formatDateTime } from "../api/time";
import type { DateTimeValue } from "../api/time";
import type { ConnectionState } from "../state/useCockpitData";
import { StatusBadge } from "./StatusBadge";

export interface RuntimeHeaderProps {
  connection: ConnectionState;
  generatedAt: DateTimeValue | null;
  runtime: RuntimeHealth | null;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function sourceStatuses(runtime: RuntimeHealth | null): string[] {
  if (!isRecord(runtime?.realtime)) {
    return [];
  }
  const state = runtime.realtime.state;
  if (!isRecord(state) || !isRecord(state.sources)) {
    return [];
  }
  return Object.values(state.sources).filter((value): value is string => typeof value === "string");
}

function statusFor(runtime: RuntimeHealth | null, connection: ConnectionState): { label: string; tone: "good" | "warn" } {
  const statuses = sourceStatuses(runtime);
  if (runtime && connection === "live" && statuses.length > 0 && statuses.every((status) => status === "fresh")) {
    return { label: "Live", tone: "good" };
  }
  return { label: "Degraded", tone: "warn" };
}

export function RuntimeHeader({ connection, generatedAt, runtime }: RuntimeHeaderProps) {
  const status = statusFor(runtime, connection);
  const sources = sourceStatuses(runtime);

  return (
    <header className="runtime-header">
      <div>
        <p className="eyebrow">Polymarket Signal Cockpit</p>
        <h1>策略驾驶舱</h1>
      </div>
      <div className="runtime-metrics" aria-label="runtime status">
        <StatusBadge label={status.label} tone={status.tone} />
        <span>WS {connection}</span>
        <span>{generatedAt ? `Updated ${formatDateTime(generatedAt)}` : "Waiting for data"}</span>
        <span>{sources.length > 0 ? sources.join(" / ") : "sources unknown"}</span>
      </div>
    </header>
  );
}
