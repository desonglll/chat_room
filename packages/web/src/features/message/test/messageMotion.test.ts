import { describe, expect, test } from 'bun:test'
import { FRESH_MESSAGE_MS, isFreshMessage } from '../MessageRow'

describe('TG-108 message entrance', () => {
  const now = Date.parse('2026-10-01T12:00:00Z')

  test('only a message that arrived moments before its row mounted rises in', () => {
    expect(isFreshMessage(new Date(now - 500).toISOString(), now)).toBe(true)
    expect(isFreshMessage(new Date(now - FRESH_MESSAGE_MS - 1).toISOString(), now)).toBe(false)
    expect(isFreshMessage('2026-09-30T12:00:00Z', now)).toBe(false)
  })

  test('a local upload is always fresh; an unreadable timestamp never animates', () => {
    expect(isFreshMessage(undefined, now)).toBe(true)
    expect(isFreshMessage('not a date', now)).toBe(false)
  })
})
