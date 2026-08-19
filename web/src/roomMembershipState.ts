import type { Room } from './types'

export function reconcileMembershipAuthFailure(room: Room, reason: string): Room {
  if (reason === 'membership required') {
    return {
      ...room,
      membership_status: undefined,
      membership_role: undefined,
    }
  }
  if (reason === 'membership pending') {
    return {
      ...room,
      membership_status: 'pending',
      membership_role: room.membership_role || 'member',
    }
  }
  return room
}
