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

/**
 * The part of the fractional second below one millisecond, in nanoseconds (0–999 999).
 * `Date.parse` stops at milliseconds, but the server stamps microseconds: two album items
 * 1 µs apart parse to the same millisecond (TG-1202), so ties break on this.
 */
function subMillisecondNs(value: string): number {
  const fraction = /T\d{2}:\d{2}:\d{2}\.(\d+)/.exec(value)?.[1] ?? ''
  return Number(fraction.slice(3, 9).padEnd(6, '0'))
}

/** Negative when `a` is earlier than `b`, positive when later, 0 when the same instant. */
export function compareInstants(a: string, b: string): number {
  return instantMs(a) - instantMs(b) || subMillisecondNs(a) - subMillisecondNs(b)
}
