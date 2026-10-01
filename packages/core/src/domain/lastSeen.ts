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
import { t } from '../i18n/t'

/** «离线» in the current language (a function: the language can change at runtime). */
export const lastSeenOfflineText = () => t('c.domain.offline')

const BUCKET_COPY = {
  get recently() {
    return t('c.domain.66b699')
  },
  get within_week() {
    return t('c.domain.2ea766')
  },
  get within_month() {
    return t('c.domain.71bcb7')
  },
  get long_ago() {
    return t('c.domain.9a7ecf')
  },
} as const

const MINUTE_MS = 60_000
const HOUR_MS = 60 * MINUTE_MS

const pad2 = (value: number) => String(value).padStart(2, '0')

const startOfLocalDay = (date: Date) => new Date(date.getFullYear(), date.getMonth(), date.getDate()).getTime()

/** `lastSeenMs` and `now` are epoch milliseconds; the calendar is the host's local zone. */
export function formatLastSeenAt(lastSeenMs: number, now: number): string {
  if (!Number.isFinite(lastSeenMs)) return lastSeenOfflineText()
  const elapsed = now - lastSeenMs
  if (elapsed < MINUTE_MS) return t('c.domain.ce5039')
  if (elapsed < HOUR_MS) return t('c.domain.ba03e1', Math.floor(elapsed / MINUTE_MS))

  const seen = new Date(lastSeenMs)
  const today = new Date(now)
  const clock = `${pad2(seen.getHours())}:${pad2(seen.getMinutes())}`
  const seenDay = startOfLocalDay(seen)
  const todayStart = startOfLocalDay(today)
  if (seenDay === todayStart) return t('c.domain.57d96f', clock)
  // Constructing "yesterday" through the calendar (not now − 24 h) survives DST shifts.
  const yesterdayStart = new Date(today.getFullYear(), today.getMonth(), today.getDate() - 1).getTime()
  if (seenDay === yesterdayStart) return t('c.domain.d47a50', clock)
  const monthDay = t('c.domain.fa0b40', seen.getMonth() + 1, seen.getDate())
  if (seen.getFullYear() === today.getFullYear()) return t('c.domain.085f4b', monthDay)
  return t('c.domain.f8c428', seen.getFullYear(), monthDay)
}

export function formatLastSeen(status: UserStatus | null | undefined, now: number): string {
  if (!status) return lastSeenOfflineText()
  switch (status.kind) {
    case 'online':
      return t('c.domain.0373ff')
    case 'offline':
      return formatLastSeenAt(Date.parse(status.last_seen), now)
    case 'recently':
    case 'within_week':
    case 'within_month':
    case 'long_ago':
      return BUCKET_COPY[status.kind]
    case 'empty':
      return lastSeenOfflineText()
  }
}
