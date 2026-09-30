/**
 * Shared fixture for the chatSession tests: fake socket, fake clock, fake drafts/REST
 * APIs and fresh stores. Pattern follows packages/core/src/realtime/chatSocket.test.ts
 * (injected fakes); stores are fresh per harness so tests never share the app
 * singletons. Not a test file itself — the two chatSession*.test.ts files consume it.
 */
import { expect } from 'bun:test'
import type { ApiClient, ChatDraft, CoreClock, CoreSocket, DraftsApi, StoredMessage } from '@tg/core'
import { createChatListStore, createComposerStore, createMessageStore, createPresenceStore } from '@tg/core'
import { createChatSession } from '../src/features/chat/chatSession'

export const CHAT_ID = 'chat-1'
export const ME = 'user-me'

export class FakeSocket implements CoreSocket {
  sent: string[] = []
  closed = false
  onopen: (() => void) | null = null
  onmessage: ((data: string) => void) | null = null
  onclose: ((info: { code: number; reason: string }) => void) | null = null
  onerror: (() => void) | null = null

  send(data: string): void {
    this.sent.push(data)
  }

  close(): void {
    if (this.closed) return
    this.closed = true
    this.onclose?.({ code: 1000, reason: '' })
  }

  open(): void {
    this.onopen?.()
  }

  receive(frame: object): void {
    this.onmessage?.(JSON.stringify(frame))
  }

  dropFromServer(): void {
    this.closed = true
    this.onclose?.({ code: 1006, reason: '' })
  }

  sentFrames(): Array<Record<string, unknown>> {
    return this.sent.map((raw) => JSON.parse(raw) as Record<string, unknown>)
  }
}

export class FakeClock implements CoreClock {
  nowMs = 0
  private timers: Array<{ id: number; at: number; callback: () => void }> = []
  private nextId = 1

  now(): number {
    return this.nowMs
  }

  setTimeout(callback: () => void, delayMs: number): unknown {
    const id = this.nextId++
    this.timers.push({ id, at: this.nowMs + delayMs, callback })
    return id
  }

  clearTimeout(handle: unknown): void {
    this.timers = this.timers.filter((timer) => timer.id !== handle)
  }

  advance(ms: number): void {
    const target = this.nowMs + ms
    for (;;) {
      const due = this.timers.filter((timer) => timer.at <= target).sort((a, b) => a.at - b.at)[0]
      if (!due) break
      this.timers = this.timers.filter((timer) => timer.id !== due.id)
      this.nowMs = due.at
      due.callback()
    }
    this.nowMs = target
  }
}

/** Drain the promise jobs a socket frame or clock tick may have queued. */
export const settle = async () => {
  for (let i = 0; i < 8; i += 1) await Promise.resolve()
}

export function storedMessage(
  id: string,
  at: string,
  content: string,
  clientMessageId: string | null = null,
): StoredMessage {
  return {
    id,
    room_id: CHAT_ID,
    client_message_id: clientMessageId,
    sender_id: 'user-other',
    sender: 'other',
    sender_avatar: '',
    content,
    attachment: null,
    reply_to: null,
    recalled_at: null,
    edited_at: null,
    created_at: at,
    favorite_id: null,
    forwarded_from: null,
    reactions: [],
  }
}

export function broadcastFrame(id: string, at: string, content: string, extra: Record<string, unknown> = {}) {
  return {
    type: 'broadcast',
    message_id: id,
    sender_id: 'user-other',
    sender: 'other',
    sender_avatar: '',
    content,
    attachment: null,
    reply_to: null,
    recalled_at: null,
    edited_at: null,
    timestamp: at,
    favorite_id: null,
    forwarded_from: null,
    reactions: [],
    ...extra,
  }
}

export const AUTH_OK = {
  type: 'auth_ok',
  room_name: '群',
  members: [
    { user_id: ME, username: 'me', avatar_emoji: '' },
    { user_id: 'user-other', username: 'other', avatar_emoji: '' },
  ],
  participants: [{ user_id: ME, username: 'me', avatar_emoji: '' }],
  read_receipts: [],
  statuses: [{ user_id: ME, status: { kind: 'online' } }],
}

export function harness(options: { storedDraft?: ChatDraft | null; missed?: StoredMessage[] } = {}) {
  const sockets: FakeSocket[] = []
  const clock = new FakeClock()
  const stores = {
    message: createMessageStore(),
    presence: createPresenceStore(),
    composer: createComposerStore(),
    chatList: createChatListStore(),
  }
  const puts: Array<{ chatId: string; text: string }> = []
  const missedCalls: number[] = []
  let putSeq = 0

  const draftsApi: DraftsApi = {
    get: async () => options.storedDraft ?? null,
    put: async (chatId, draft) => {
      puts.push({ chatId, text: draft.text })
      putSeq += 1
      return {
        user_id: ME,
        text: draft.text,
        reply_to_message_id: draft.reply_to_message_id ?? null,
        topic_id: draft.topic_id ?? null,
        updated_at: `2026-09-30T10:00:0${putSeq}Z`,
      }
    },
  }

  // The session only calls `client.json` (via listChatMessages) for reconnect catch-up.
  const client = {
    json: async () => {
      missedCalls.push(clock.nowMs)
      return options.missed ?? []
    },
    request: async () => new Response(),
  } as unknown as ApiClient

  const session = createChatSession({
    chatId: CHAT_ID,
    token: 'token-1',
    currentUserId: ME,
    socketUrl: 'ws://test/ws/chat-1',
    createSocket: (url) => {
      expect(url).toBe('ws://test/ws/chat-1')
      const socket = new FakeSocket()
      sockets.push(socket)
      return socket
    },
    clock,
    client,
    draftsApi,
    stores,
  })

  const online = async () => {
    session.start()
    await settle()
    sockets[0]!.open()
    sockets[0]!.receive(AUTH_OK)
    sockets[0]!.receive({ type: 'history_complete' })
    await settle()
  }

  return { session, sockets, clock, stores, puts, missedCalls, online }
}
