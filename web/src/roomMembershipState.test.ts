import { describe, expect, test } from 'bun:test'
import { canAutoConnectRoom, reconcileMembershipAuthFailure, shouldAutoConnectRoom } from './roomMembershipState'
import type { Room, User } from './types'

const activeRoom: Room = {
  id: 'room-1',
  name: 'Review room',
  has_password: false,
  creator_user_id: 'owner-1',
  join_policy: 'approval',
  avatar_emoji: '',
  description: '',
  membership_status: 'active',
  membership_role: 'member',
  unread_count: 0,
  created_at: '2026-08-19T00:00:00Z',
}

const currentUser: User = {
  id: 'user-1',
  username: 'mike',
  display_name: 'Mike',
  avatar_emoji: '',
  signature: '',
  homepage: '',
  created_at: '2026-08-19T00:00:00Z',
}

describe('automatic room reconnection', () => {
  test('reconnects active public rooms for a restored account session', () => {
    expect(canAutoConnectRoom(activeRoom, currentUser, 'session-token', '')).toBe(true)
  })

  test('requires a stored room password before reconnecting a private room', () => {
    const privateRoom = { ...activeRoom, has_password: true }
    expect(canAutoConnectRoom(privateRoom, currentUser, 'session-token', '')).toBe(false)
    expect(canAutoConnectRoom(privateRoom, currentUser, 'session-token', 'room-password')).toBe(true)
  })

  test('does not reconnect without active membership and an account session', () => {
    expect(canAutoConnectRoom({ ...activeRoom, membership_status: 'pending' }, currentUser, 'session-token', '')).toBe(
      false,
    )
    expect(canAutoConnectRoom(activeRoom, null, '', '')).toBe(false)
  })

  test('retries a restored active room only while its socket is idle', () => {
    expect(shouldAutoConnectRoom(activeRoom, currentUser, 'session-token', '', 'idle')).toBe(true)
    expect(shouldAutoConnectRoom(activeRoom, currentUser, 'session-token', '', 'connecting')).toBe(false)
    expect(shouldAutoConnectRoom(activeRoom, currentUser, 'session-token', '', 'online')).toBe(false)
  })
})

describe('room membership authentication failures', () => {
  test('restores the application action when the server has no membership', () => {
    const room = reconcileMembershipAuthFailure(activeRoom, 'membership required')
    expect(room.membership_status).toBeUndefined()
    expect(room.membership_role).toBeUndefined()
  })

  test('shows the waiting state when an application is already pending', () => {
    const room = reconcileMembershipAuthFailure(activeRoom, 'membership pending')
    expect(room.membership_status).toBe('pending')
    expect(room.membership_role).toBe('member')
  })

  test('does not alter membership for unrelated authentication errors', () => {
    expect(reconcileMembershipAuthFailure(activeRoom, 'wrong password')).toBe(activeRoom)
  })
})
