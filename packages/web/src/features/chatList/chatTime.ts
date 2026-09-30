/**
 * The chat-list timestamp, Telegram's rule: `HH:mm` for today, the weekday within the
 * last week, a date otherwise (with the year only when it is not this year).
 * Local time, pure: `now` is injected so tests and the row render agree on "today".
 */

const WEEKDAYS = ['周日', '周一', '周二', '周三', '周四', '周五', '周六'] as const
const DAY_MS = 86_400_000

const pad = (value: number): string => String(value).padStart(2, '0')

const startOfDay = (date: Date): number => new Date(date.getFullYear(), date.getMonth(), date.getDate()).getTime()

export function formatChatListTime(value: string, now: Date): string {
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return ''
  // Rounded: a DST day is 23 or 25 hours long, never 24.
  const daysAgo = Math.round((startOfDay(now) - startOfDay(date)) / DAY_MS)
  // Today, and a small future skew between server and client clocks, both read as a time.
  if (daysAgo <= 0 && daysAgo > -2) return `${pad(date.getHours())}:${pad(date.getMinutes())}`
  if (daysAgo > 0 && daysAgo < 7) return WEEKDAYS[date.getDay()] ?? ''
  const month = date.getMonth() + 1
  if (date.getFullYear() === now.getFullYear()) return `${month}月${date.getDate()}日`
  return `${date.getFullYear()}/${month}/${date.getDate()}`
}
