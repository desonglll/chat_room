/**
 * TG-906: comparing wire timestamps as instants. RFC 3339 strings only sort lexically when
 * they share one offset and one fraction width; the server's `2026-10-01T09:00:00Z` and
 * `2026-10-01T09:00:00.5Z`, or a `+08:00` value, would compare wrongly as strings. Unparseable
 * input compares as the epoch, i.e. oldest.
 */
export function instantMs(value: string): number {
  const ms = Date.parse(value)
  return Number.isFinite(ms) ? ms : 0
}

/** Negative when `a` is earlier than `b`, positive when later, 0 when the same instant. */
export function compareInstants(a: string, b: string): number {
  return instantMs(a) - instantMs(b)
}
