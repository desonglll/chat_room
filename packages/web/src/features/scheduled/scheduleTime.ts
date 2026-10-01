/**
 * TG-404 date helpers for scheduling, pure and local-time based (the picker is a native
 * `datetime-local` input, which speaks local wall-clock time without a zone).
 */
import { SCHEDULED_LIMITS } from '@tg/core'
import { t } from '../../i18n/index'

const MINUTE = 60_000
const DAY = 24 * 60 * MINUTE

const pad = (value: number): string => String(value).padStart(2, '0')

/** `YYYY-MM-DDTHH:MM` in local time — the `datetime-local` input's value format. */
export function toLocalInputValue(date: Date): string {
  return (
    `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}` +
    `T${pad(date.getHours())}:${pad(date.getMinutes())}`
  )
}

/** Parse a `datetime-local` value as local time; null when malformed. */
export function fromLocalInputValue(value: string): Date | null {
  const match = /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2})$/.exec(value)
  if (!match) return null
  const [, year, month, day, hours, minutes] = match.map(Number) as [number, number, number, number, number, number]
  const date = new Date(year, month - 1, day, hours, minutes, 0, 0)
  return date.getMonth() === month - 1 && date.getDate() === day ? date : null
}

/** Telegram's default: one hour from now, rounded up to the next five minutes. */
export function defaultScheduleTime(now: Date): Date {
  const next = new Date(now.getTime() + 60 * MINUTE)
  next.setSeconds(0, 0)
  const remainder = next.getMinutes() % 5
  if (remainder !== 0) next.setMinutes(next.getMinutes() + 5 - remainder)
  return next
}

/** Error copy for an unusable time, or null when the server will accept it. */
export function scheduleTimeError(at: Date | null, now: Date): string | null {
  if (!at) return t('w.scheduled.dd3427')
  if (at.getTime() <= now.getTime()) return t('w.scheduled.7b61fc')
  if (at.getTime() > now.getTime() + SCHEDULED_LIMITS.maxDaysAhead * DAY) return t('w.scheduled.593a65')
  return null
}

function startOfDay(date: Date): number {
  return new Date(date.getFullYear(), date.getMonth(), date.getDate()).getTime()
}

/** «今天» / «明天» / «10月3日» / «2027年1月5日». */
export function formatScheduleDay(at: Date, now: Date): string {
  const days = Math.round((startOfDay(at) - startOfDay(now)) / DAY)
  if (days === 0) return t('w.scheduled.17e83c')
  if (days === 1) return t('w.scheduled.b76ce2')
  const monthDay = t('w.scheduled.fa0b40', at.getMonth() + 1, at.getDate())
  return at.getFullYear() === now.getFullYear() ? monthDay : t('w.scheduled.a8571e', at.getFullYear(), monthDay)
}

export function formatScheduleClock(at: Date): string {
  return `${pad(at.getHours())}:${pad(at.getMinutes())}`
}

/** The send button's confirmation label: «今天 21:00 发送». */
export function formatScheduleLabel(at: Date, now: Date): string {
  return t('w.scheduled.275533', formatScheduleDay(at, now), formatScheduleClock(at))
}

export interface DayGroup<T> {
  day: string
  items: T[]
}

/** Consecutive items (already sorted by time) grouped under their day heading. */
export function groupByDay<T extends { scheduled_at: string }>(items: readonly T[], now: Date): DayGroup<T>[] {
  const groups: DayGroup<T>[] = []
  for (const item of items) {
    const day = formatScheduleDay(new Date(item.scheduled_at), now)
    const last = groups[groups.length - 1]
    if (last && last.day === day) last.items.push(item)
    else groups.push({ day, items: [item] })
  }
  return groups
}
