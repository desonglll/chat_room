// TG-404: local-time helpers for the schedule picker and the list's day headings.
import { describe, expect, test } from 'bun:test'
import {
  defaultScheduleTime,
  formatScheduleDay,
  formatScheduleLabel,
  fromLocalInputValue,
  groupByDay,
  scheduleTimeError,
  toLocalInputValue,
} from '../scheduleTime'

const at = (y: number, m: number, d: number, h = 0, min = 0) => new Date(y, m - 1, d, h, min)

describe('schedule time', () => {
  test('datetime-local values round-trip in local time and reject nonsense', () => {
    const date = at(2026, 10, 1, 9, 5)
    expect(toLocalInputValue(date)).toBe('2026-10-01T09:05')
    expect(fromLocalInputValue('2026-10-01T09:05')?.getTime()).toBe(date.getTime())
    expect(fromLocalInputValue('2026-02-30T09:05')).toBeNull()
    expect(fromLocalInputValue('tomorrow')).toBeNull()
  })

  test('the default is an hour ahead, rounded up to five minutes', () => {
    expect(toLocalInputValue(defaultScheduleTime(at(2026, 10, 1, 9, 2)))).toBe('2026-10-01T10:05')
    expect(toLocalInputValue(defaultScheduleTime(at(2026, 10, 1, 23, 55)))).toBe('2026-10-02T00:55')
  })

  test('only future times within a year are accepted', () => {
    const now = at(2026, 10, 1, 12)
    expect(scheduleTimeError(null, now)).toBe('请选择日期和时间')
    expect(scheduleTimeError(at(2026, 10, 1, 11, 59), now)).toBe('请选择将来的时间')
    expect(scheduleTimeError(at(2026, 10, 1, 12, 1), now)).toBeNull()
    expect(scheduleTimeError(at(2027, 10, 10), now)).toBe('最多只能提前一年定时')
  })

  test('days read as 今天 / 明天 / a date, with the year only when it differs', () => {
    const now = at(2026, 10, 1, 23)
    expect(formatScheduleDay(at(2026, 10, 1, 23, 30), now)).toBe('今天')
    expect(formatScheduleDay(at(2026, 10, 2, 0, 10), now)).toBe('明天')
    expect(formatScheduleDay(at(2026, 12, 25), now)).toBe('12月25日')
    expect(formatScheduleDay(at(2027, 1, 5), now)).toBe('2027年1月5日')
    expect(formatScheduleLabel(at(2026, 10, 2, 9), now)).toBe('明天 09:00 发送')
  })

  test('items group under consecutive day headings', () => {
    const now = at(2026, 10, 1, 8)
    const items = [
      { id: 'a', scheduled_at: at(2026, 10, 1, 9).toISOString() },
      { id: 'b', scheduled_at: at(2026, 10, 1, 21).toISOString() },
      { id: 'c', scheduled_at: at(2026, 10, 2, 9).toISOString() },
    ]
    expect(groupByDay(items, now).map((group) => [group.day, group.items.map((item) => item.id)])).toEqual([
      ['今天', ['a', 'b']],
      ['明天', ['c']],
    ])
  })
})
