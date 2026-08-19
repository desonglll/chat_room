import { describe, expect, test } from 'bun:test'
import { reconcileMembershipAuthFailure } from './roomMembershipState'
import type { Room } from './types'

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
