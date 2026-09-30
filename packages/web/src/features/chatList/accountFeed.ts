/**
 * The account socket (`/ws/account`) → `chatListStore`: other members' new messages in
 * every chat, and the unread snapshot. Framework-free with an injected socket factory
 * and clock, so reconnect behaviour is testable with fakes.
 *
 * The server never echoes the caller's own messages here; `timelineMirror.ts` covers
 * those from the open chat's timeline.
 */
import type { AccountChatState, AccountMessageEvent, ChatListStore, CoreClock, CoreSocketFactory } from '@tg/core'
import { reconnectDelayMs } from '@tg/core'

export interface AccountFeedOptions {
  url: string
  token: string
  socketFactory: CoreSocketFactory
  clock: CoreClock
  store: ChatListStore
  activeChatId: () => string
  /** Reload the list: after a reconnect (missed events) and when an unknown chat appears. */
  resync: () => void
}

type AccountFrame = AccountMessageEvent | { type: 'unread_counts'; chats: AccountChatState[] } | { type: string }

function parseFrame(data: string): AccountFrame | null {
  try {
    const parsed = JSON.parse(data) as AccountFrame | null
    return typeof parsed === 'object' && parsed !== null && typeof parsed.type === 'string' ? parsed : null
  } catch {
    return null
  }
}

/** Starts the feed; the returned function stops it for good. */
export function startAccountFeed(options: AccountFeedOptions): () => void {
  let stopped = false
  let attempt = 0
  let timer: unknown = null
  let socket: ReturnType<CoreSocketFactory> | null = null

  const handle = (frame: AccountFrame) => {
    const state = options.store.getState()
    if (frame.type === 'new_message') {
      const event = frame as AccountMessageEvent
      if (!state.conversations.some((conversation) => conversation.room_id === event.room_id)) options.resync()
      else state.applyAccountMessage(event, options.activeChatId())
    } else if (frame.type === 'unread_counts') {
      const chats = (frame as { chats: AccountChatState[] }).chats
      const known = new Set(state.conversations.map((conversation) => conversation.room_id))
      const unknown = chats.some((entry) => entry.membership_status === 'active' && !known.has(entry.room_id))
      state.applyUnreadCounts(chats)
      if (unknown) options.resync()
    }
  }

  const connect = () => {
    if (stopped) return
    const current = options.socketFactory(options.url)
    socket = current
    current.onopen = () => {
      current.send(JSON.stringify({ token: options.token }))
      if (attempt > 0) options.resync()
      attempt = 0
    }
    current.onmessage = (data) => {
      const frame = parseFrame(data)
      if (frame) handle(frame)
    }
    current.onclose = () => {
      if (stopped || socket !== current) return
      timer = options.clock.setTimeout(connect, reconnectDelayMs(attempt))
      attempt += 1
    }
    current.onerror = null
  }

  connect()
  return () => {
    stopped = true
    if (timer !== null) options.clock.clearTimeout(timer)
    socket?.close(1000, 'client closed')
    socket = null
  }
}
