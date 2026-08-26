import { describe, expect, test } from 'bun:test'
import { shouldReconnectRestoredRoom } from './useAppBootstrap'

describe('restored room routing', () => {
  test('reconnects a room that was connected before refresh', () => {
    expect(shouldReconnectRestoredRoom('room', 'room-1', 'room-1')).toBe(true)
  })

  test('keeps an explicit join route behind the join gate', () => {
    expect(shouldReconnectRestoredRoom('room-join', 'room-1', 'room-1')).toBe(false)
  })

  test('does not reconnect a stale cached selection for another route', () => {
    expect(shouldReconnectRestoredRoom('room', 'room-1', 'room-2')).toBe(false)
  })
})
