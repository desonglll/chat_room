import { describe, expect, test } from 'bun:test'
import { resolveRoomViewState } from './roomViewState'
import type { Room } from './types'

const room: Room = {
  id: 'room-1',
  name: 'General',
  has_password: false,
  creator_user_id: 'user-1',
  join_policy: 'open',
  avatar_emoji: '',
  description: '',
  membership_status: 'active',
  membership_role: 'member',
  unread_count: 0,
  created_at: '2026-08-19T00:00:00Z',
}

describe('resolveRoomViewState', () => {
  test('keeps the join form out of an active room reconnect', () => {
    expect(
      resolveRoomViewState({ room, status: 'connecting', authenticated: false, loading: false, messageCount: 0 }),
    ).toBe('connecting')
  })

  test('does not ask an active member to join again before reconnection starts', () => {
    expect(resolveRoomViewState({ room, status: 'idle', authenticated: false, loading: false, messageCount: 0 })).toBe(
      'connecting',
    )
  })

  test('preserves messages while reconnecting or offline', () => {
    expect(
      resolveRoomViewState({ room, status: 'offline', authenticated: false, loading: false, messageCount: 4 }),
    ).toBe('conversation')
  })

  test('shows access controls for a room without active membership', () => {
    expect(
      resolveRoomViewState({
        room: { ...room, membership_status: undefined },
        status: 'idle',
        authenticated: false,
        loading: false,
        messageCount: 0,
      }),
    ).toBe('access')
  })

  test('distinguishes cold loading from an empty selection', () => {
    expect(
      resolveRoomViewState({ room: null, status: 'idle', authenticated: false, loading: true, messageCount: 0 }),
    ).toBe('loading')
    expect(
      resolveRoomViewState({ room: null, status: 'idle', authenticated: false, loading: false, messageCount: 0 }),
    ).toBe('empty')
  })
})
