import { onBeforeUnmount } from 'vue'
import type { AccountMessageEvent } from '../types'

interface UnreadSnapshot {
  type: 'unread_counts'
  rooms: {
    room_id: string
    unread_count: number
    membership_status: 'pending' | 'invited' | 'active'
    membership_role: 'owner' | 'admin' | 'member'
  }[]
}

export function useUnreadSocket(
  onSnapshot: (counts: Map<string, UnreadSnapshot['rooms'][number]>) => void,
  onMessage: (message: AccountMessageEvent) => void,
) {
  let socket: WebSocket | null = null
  let reconnectTimer: number | undefined
  let activeToken = ''
  let attempts = 0

  function clearTimer(): void {
    window.clearTimeout(reconnectTimer)
    reconnectTimer = undefined
  }

  function close(): void {
    activeToken = ''
    clearTimer()
    if (socket) {
      socket.onclose = null
      socket.close()
      socket = null
    }
  }

  function open(token: string): void {
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
    const next = new WebSocket(`${protocol}//${window.location.host}/ws/account`)
    socket = next
    next.onopen = () => {
      if (socket !== next) return
      attempts = 0
      next.send(JSON.stringify({ token }))
    }
    next.onmessage = (event: MessageEvent<string>) => {
      if (socket !== next) return
      try {
        const message = JSON.parse(event.data) as UnreadSnapshot | AccountMessageEvent
        if (message.type === 'unread_counts') {
          onSnapshot(new Map(message.rooms.map((room) => [room.room_id, room])))
        }
        if (message.type === 'new_message') onMessage(message)
      } catch {
        // Ignore malformed account events and keep the room connection alive.
      }
    }
    next.onclose = () => {
      if (socket !== next) return
      socket = null
      if (!activeToken) return
      const delay = Math.min(500 * (2 ** attempts++), 5000)
      reconnectTimer = window.setTimeout(() => open(activeToken), delay)
    }
  }

  function connect(token: string): void {
    close()
    if (!token) return
    activeToken = token
    open(token)
  }

  onBeforeUnmount(close)
  return { close, connect }
}
