// TG-107: last-seen wording boundaries. Times are built in the host's local zone so the
// calendar-day assertions hold in every TZ the suite runs under.
import { describe, expect, test } from 'bun:test'
import { formatLastSeen, formatLastSeenAt } from './lastSeen'

const at = (year: number, month: number, day: number, hour = 0, minute = 0, second = 0) =>
  new Date(year, month - 1, day, hour, minute, second).getTime()

const NOW = at(2026, 9, 30, 15, 30)
const offline = (ms: number) => ({ kind: 'offline' as const, last_seen: new Date(ms).toISOString() })

describe('formatLastSeenAt', () => {
  test('under a minute (and a skewed future stamp) is 刚刚上线', () => {
    expect(formatLastSeenAt(NOW, NOW)).toBe('刚刚上线')
    expect(formatLastSeenAt(NOW - 59_999, NOW)).toBe('刚刚上线')
    expect(formatLastSeenAt(NOW + 30_000, NOW)).toBe('刚刚上线')
  })

  test('one to fifty-nine minutes counts minutes', () => {
    expect(formatLastSeenAt(NOW - 60_000, NOW)).toBe('1 分钟前上线')
    expect(formatLastSeenAt(NOW - 119_999, NOW)).toBe('1 分钟前上线')
    expect(formatLastSeenAt(NOW - 59 * 60_000 - 59_999, NOW)).toBe('59 分钟前上线')
  })

  test('an hour or more on the same day shows today with the clock time', () => {
    expect(formatLastSeenAt(NOW - 60 * 60_000, NOW)).toBe('今天 14:30 上线')
    expect(formatLastSeenAt(at(2026, 9, 30, 0, 0), NOW)).toBe('今天 00:00 上线')
  })

  test('the previous calendar day is 昨天', () => {
    expect(formatLastSeenAt(at(2026, 9, 29, 23, 59), NOW)).toBe('昨天 23:59 上线')
    expect(formatLastSeenAt(at(2026, 9, 29, 0, 5), NOW)).toBe('昨天 00:05 上线')
    // Just after midnight a 40-minute gap is still minutes, not 昨天.
    expect(formatLastSeenAt(at(2026, 9, 29, 23, 50), at(2026, 9, 30, 0, 30))).toBe('40 分钟前上线')
    // Two hours across midnight is 昨天.
    expect(formatLastSeenAt(at(2026, 9, 29, 22, 30), at(2026, 9, 30, 0, 30))).toBe('昨天 22:30 上线')
  })

  test('older dates show the date, with the year only when it differs', () => {
    expect(formatLastSeenAt(at(2026, 9, 28, 23, 59), NOW)).toBe('9月28日上线')
    expect(formatLastSeenAt(at(2026, 1, 1, 8, 0), NOW)).toBe('1月1日上线')
    expect(formatLastSeenAt(at(2025, 12, 31, 23, 0), NOW)).toBe('2025年12月31日上线')
    expect(formatLastSeenAt(at(2025, 12, 31, 23, 0), at(2026, 1, 1, 9, 0))).toBe('昨天 23:00 上线')
  })
})

describe('formatLastSeen', () => {
  test('maps every UserStatus kind', () => {
    expect(formatLastSeen({ kind: 'online' }, NOW)).toBe('在线')
    expect(formatLastSeen(offline(NOW - 5 * 60_000), NOW)).toBe('5 分钟前上线')
    expect(formatLastSeen({ kind: 'recently' }, NOW)).toBe('最近上线')
    expect(formatLastSeen({ kind: 'within_week' }, NOW)).toBe('本周内上线')
    expect(formatLastSeen({ kind: 'within_month' }, NOW)).toBe('本月内上线')
    expect(formatLastSeen({ kind: 'long_ago' }, NOW)).toBe('很久以前上线')
    expect(formatLastSeen({ kind: 'empty' }, NOW)).toBe('离线')
    expect(formatLastSeen(undefined, NOW)).toBe('离线')
    expect(formatLastSeen({ kind: 'offline', last_seen: 'not a date' }, NOW)).toBe('离线')
  })

  test('relative text advances as the clock moves', () => {
    const status = offline(NOW)
    expect(formatLastSeen(status, NOW + 30_000)).toBe('刚刚上线')
    expect(formatLastSeen(status, NOW + 60_000)).toBe('1 分钟前上线')
    expect(formatLastSeen(status, NOW + 3 * 60 * 60_000)).toBe('今天 15:30 上线')
  })
})
