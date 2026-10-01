import { describe, expect, test } from 'bun:test'
import { approximatePoint, isLiveLocation, liveRemaining } from './locations'

const at = (iso: string) => Date.parse(iso)

describe('TG-407 location rules', () => {
  const live = { latitude: 1, longitude: 2, updated_at: '', live_until: '2026-10-01T12:30:00Z' }

  test('a live location is live only before its end', () => {
    expect(isLiveLocation(live, at('2026-10-01T12:00:00Z'))).toBe(true)
    expect(isLiveLocation(live, at('2026-10-01T12:30:00Z'))).toBe(false)
    expect(isLiveLocation({ latitude: 1, longitude: 2, updated_at: '' }, 0)).toBe(false)
  })

  test('remaining time reads in minutes, then hours, and is empty once ended', () => {
    expect(liveRemaining(live, at('2026-10-01T12:18:00Z'))).toBe('剩余 12 分钟')
    expect(liveRemaining({ ...live, live_until: '2026-10-01T20:00:00Z' }, at('2026-10-01T12:00:00Z'))).toBe(
      '剩余 8 小时',
    )
    expect(liveRemaining(live, at('2026-10-01T13:00:00Z'))).toBe('')
  })

  test('an approximate point is rounded to ~1 km and carries no heading', () => {
    expect(approximatePoint({ latitude: 31.230416, longitude: 121.473701, accuracy_m: 5, heading: 90 })).toEqual({
      latitude: 31.23,
      longitude: 121.47,
      accuracy_m: 1000,
    })
  })
})
