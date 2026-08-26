import type { ChatStatus, Room } from './types'

export type RoomViewState = 'loading' | 'empty' | 'connecting' | 'access' | 'conversation'

interface RoomViewInput {
  room: Room | null
  status: ChatStatus
  authenticated: boolean
  loading: boolean
  messageCount: number
}

export function resolveRoomViewState(input: RoomViewInput): RoomViewState {
  if (!input.room) return input.loading ? 'loading' : 'empty'
  if (input.authenticated) return 'conversation'
  if (input.messageCount > 0 && ['connecting', 'offline'].includes(input.status)) return 'conversation'
  if (input.room.membership_status === 'active' && ['idle', 'connecting', 'offline'].includes(input.status)) {
    return 'connecting'
  }
  return 'access'
}
