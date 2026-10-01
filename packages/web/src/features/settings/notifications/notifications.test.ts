import { describe, expect, test } from 'bun:test'
import { mutePatch } from '../../chatInfo/NotificationRow'

describe('TG-508 mute durations', () => {
  test('timed mutes set muted_until; forever is level none; unmute clears both', () => {
    const now = Date.parse('2026-10-01T00:00:00Z')
    expect(mutePatch(1, now)).toEqual({ notification_level: 'all', muted_until: '2026-10-01T01:00:00.000Z' })
    expect(mutePatch(48, now).muted_until).toBe('2026-10-03T00:00:00.000Z')
    expect(mutePatch(null, now)).toEqual({ notification_level: 'none', muted_until: null })
    expect(mutePatch(0, now)).toEqual({ notification_level: 'all', muted_until: null })
  })
})
