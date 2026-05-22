export type DateTimeValue = string | number[];

export function dateTimeToMillis(value: DateTimeValue | null | undefined): number | null {
  if (!value) {
    return null;
  }
  if (typeof value === "string") {
    const parsed = Date.parse(value);
    return Number.isNaN(parsed) ? null : parsed;
  }
  const [year, ordinal, hour = 0, minute = 0, second = 0, nanosecond = 0] = value;
  if (typeof year !== "number" || typeof ordinal !== "number") {
    return null;
  }
  return Date.UTC(year, 0, ordinal, hour, minute, second, Math.floor(nanosecond / 1_000_000));
}

export function formatDateTime(value: DateTimeValue | null | undefined): string {
  const millis = dateTimeToMillis(value);
  return millis === null ? "-" : new Date(millis).toLocaleTimeString();
}
