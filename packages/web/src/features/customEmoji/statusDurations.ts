/** The expiry choices of the emoji status picker (Telegram's set). `null` = until cleared. */
export interface StatusDuration {
  label: string
  ms: number | null
}

export const STATUS_DURATIONS: readonly StatusDuration[] = [
  { label: '永久', ms: null },
  { label: '1 小时', ms: 3_600_000 },
  { label: '8 小时', ms: 8 * 3_600_000 },
  { label: '1 天', ms: 24 * 3_600_000 },
  { label: '1 周', ms: 7 * 24 * 3_600_000 },
]

export function expiryFor(duration: StatusDuration, now: number): string | null {
  return duration.ms === null ? null : new Date(now + duration.ms).toISOString()
}
