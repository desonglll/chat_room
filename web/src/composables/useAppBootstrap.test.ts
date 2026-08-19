import { describe, expect, test } from 'bun:test'
import { shouldReconnectRestoredRoom } from './useAppBootstrap'

describe('restored room routing', () => {
  test('reconnects a room that was connected before refresh', () => {
    expect(shouldReconnectRestoredRoom('room')).toBe(true)
  })

  test('keeps an explicit join route behind the join gate', () => {
    expect(shouldReconnectRestoredRoom('room-join')).toBe(false)
  })
})
