import { describe, expect, test } from 'bun:test'
import { shouldPromoteJoinRoute } from './useRoomRouteSync'

describe('room route synchronization', () => {
  test('never redirects a room to join during refresh reconnection', () => {
    expect(shouldPromoteJoinRoute(false, 'room')).toBe(false)
    expect(shouldPromoteJoinRoute(false, 'room-join')).toBe(false)
  })

  test('promotes the join route only after room authentication succeeds', () => {
    expect(shouldPromoteJoinRoute(true, 'room-join')).toBe(true)
    expect(shouldPromoteJoinRoute(true, 'room')).toBe(false)
  })
})
