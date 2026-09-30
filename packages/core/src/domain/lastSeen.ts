/**
 * TG-107: Telegram's "last seen" wording for a user status (TG-007 `UserStatus`).
 *
 * Exact-time rules (tdesktop `OnlineText` / Android `formatDateOnline`), evaluated in this
 * order against the host's local calendar:
 *   online                → 在线
 *   < 1 min ago (or future, i.e. clock skew) → 刚刚上线
 *   < 60 min ago          → N 分钟前上线
 *   same calendar day     → 今天 HH:mm 上线
 *   previous calendar day → 昨天 HH:mm 上线
 *   same year             → M月D日上线
 *   older                 → YYYY年M月D日上线
 * Privacy buckets (TG-505 placeholders already on the wire): recently → 最近上线,
 * within_week → 本周内上线, within_month → 本月内上线, long_ago → 很久以前上线.
 * `empty` (and no status at all) → 离线: TG-007's server sends `empty` when it simply has no
 * persisted last-seen, so claiming "a long time ago" would state something untrue.
 */
import type { UserStatus } from '../types'

export const LAST_SEEN_OFFLINE_TEXT = '离线'

const BUCKET_COPY = {
  recently: '最近上线',
  within_week: '本周内上线',
  within_month: '本月内上线',
  long_ago: '很久以前上线',
} as const

const MINUTE_MS = 60_000
const HOUR_MS = 60 * MINUTE_MS

const pad2 = (value: number) => String(value).padStart(2, '0')

const startOfLocalDay = (date: Date) => new Date(date.getFullYear(), date.getMonth(), date.getDate()).getTime()

/** `lastSeenMs` and `now` are epoch milliseconds; the calendar is the host's local zone. */
export function formatLastSeenAt(lastSeenMs: number, now: number): string {
  if (!Number.isFinite(lastSeenMs)) return LAST_SEEN_OFFLINE_TEXT
  const elapsed = now - lastSeenMs
  if (elapsed < MINUTE_MS) return '刚刚上线'
  if (elapsed < HOUR_MS) return `${Math.floor(elapsed / MINUTE_MS)} 分钟前上线`

  const seen = new Date(lastSeenMs)
  const today = new Date(now)
  const clock = `${pad2(seen.getHours())}:${pad2(seen.getMinutes())}`
  const seenDay = startOfLocalDay(seen)
  const todayStart = startOfLocalDay(today)
  if (seenDay === todayStart) return `今天 ${clock} 上线`
  // Constructing "yesterday" through the calendar (not now − 24 h) survives DST shifts.
  const yesterdayStart = new Date(today.getFullYear(), today.getMonth(), today.getDate() - 1).getTime()
  if (seenDay === yesterdayStart) return `昨天 ${clock} 上线`
  const monthDay = `${seen.getMonth() + 1}月${seen.getDate()}日`
  if (seen.getFullYear() === today.getFullYear()) return `${monthDay}上线`
  return `${seen.getFullYear()}年${monthDay}上线`
}

export function formatLastSeen(status: UserStatus | null | undefined, now: number): string {
  if (!status) return LAST_SEEN_OFFLINE_TEXT
  switch (status.kind) {
    case 'online':
      return '在线'
    case 'offline':
      return formatLastSeenAt(Date.parse(status.last_seen), now)
    case 'recently':
    case 'within_week':
    case 'within_month':
    case 'long_ago':
      return BUCKET_COPY[status.kind]
    case 'empty':
      return LAST_SEEN_OFFLINE_TEXT
  }
}
