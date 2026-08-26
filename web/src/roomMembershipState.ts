import type { ChatStatus, Room, User } from './types'

export function canAutoConnectRoom(room: Room, user: User | null, token: string, password: string): boolean {
  return room.membership_status === 'active' && Boolean(user && token) && (!room.has_password || Boolean(password))
}

export function shouldAutoConnectRoom(
  room: Room | null,
  user: User | null,
  token: string,
  password: string,
  status: ChatStatus,
): room is Room {
  return status === 'idle' && Boolean(room && canAutoConnectRoom(room, user, token, password))
}

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
