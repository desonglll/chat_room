import { afterEach, describe, expect, spyOn, test } from 'bun:test'
import { useChatSocket } from './useChatSocket'
import type { BroadcastMessage, Room } from '../types'

class FakeWebSocket {
  static readonly OPEN = 1
  static instances: FakeWebSocket[] = []

  onopen: (() => void) | null = null
  onmessage: ((event: MessageEvent<string>) => void) | null = null
  onerror: (() => void) | null = null
  onclose: (() => void) | null = null
  readyState = FakeWebSocket.OPEN
  sent: string[] = []

  constructor(readonly url: string) {
    FakeWebSocket.instances.push(this)
  }

  close(): void {}
  send(payload: string): void {
    this.sent.push(payload)
  }
}

const originalWindow = globalThis.window
const originalWebSocket = globalThis.WebSocket

afterEach(() => {
  FakeWebSocket.instances = []
  Object.defineProperty(globalThis, 'window', { configurable: true, value: originalWindow })
  Object.defineProperty(globalThis, 'WebSocket', { configurable: true, value: originalWebSocket })
})

describe('room socket switching', () => {
  test('ignores an error emitted by a room socket after that socket was closed', () => {
    Object.defineProperty(globalThis, 'window', {
      configurable: true,
      value: {
        clearTimeout,
        crypto: globalThis.crypto,
        location: { host: 'localhost:3000', protocol: 'http:' },
        setTimeout,
      },
    })
    Object.defineProperty(globalThis, 'WebSocket', { configurable: true, value: FakeWebSocket })
    const room: Room = {
      id: 'room-1',
      name: '旧聊天室',
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
    const warning = spyOn(console, 'warn').mockImplementation(() => {})
    const chat = useChatSocket()
    warning.mockRestore()

    chat.connect(room, 'session-token', 'user-1', '')
    const staleError = FakeWebSocket.instances[0]?.onerror
    chat.close()
    staleError?.()

    expect(chat.status.value).toBe('idle')
    expect(chat.error.value).toBe('')
  })

  test('merges authoritative history into an existing message after reconnect', () => {
    Object.defineProperty(globalThis, 'window', {
      configurable: true,
      value: {
        clearTimeout,
        crypto: globalThis.crypto,
        location: { host: 'localhost:3000', protocol: 'http:' },
        setTimeout,
      },
    })
    Object.defineProperty(globalThis, 'WebSocket', { configurable: true, value: FakeWebSocket })
    const room: Room = {
      id: 'room-1',
      name: '聊天室',
      has_password: false,
      creator_user_id: 'user-1',
      join_policy: 'open',
      avatar_emoji: '',
      description: '',
      membership_status: 'active',
      membership_role: 'member',
      unread_count: 0,
      created_at: '2026-08-20T00:00:00Z',
    }
    const original: BroadcastMessage = {
      type: 'broadcast',
      message_id: 'message-1',
      sender_id: 'user-2',
      sender: 'friend',
      sender_avatar: '',
      content: 'hello',
      attachment: null,
      reply_to: null,
      recalled_at: null,
      edited_at: null,
      timestamp: '2026-08-20T00:00:01Z',
      forwarded_from: null,
      reactions: [],
    }
    const warning = spyOn(console, 'warn').mockImplementation(() => {})
    const chat = useChatSocket()
    warning.mockRestore()
    chat.connect(room, 'session-token', 'user-1', '')
    const socket = FakeWebSocket.instances[0]
    socket?.onmessage?.({ data: JSON.stringify({ type: 'auth_ok' }) } as MessageEvent<string>)
    socket?.onmessage?.({ data: JSON.stringify(original) } as MessageEvent<string>)
    socket?.onmessage?.({
      data: JSON.stringify({ ...original, reactions: [{ emoji: '👍', user_ids: ['user-2'] }] }),
    } as MessageEvent<string>)

    expect((chat.messages.value[0] as BroadcastMessage).reactions).toEqual([{ emoji: '👍', user_ids: ['user-2'] }])
    chat.close()
  })

  test('sends messages when randomUUID is unavailable on an insecure origin', () => {
    Object.defineProperty(globalThis, 'window', {
      configurable: true,
      value: {
        clearTimeout,
        crypto: {
          getRandomValues(values: Uint8Array) {
            values.fill(0x11)
            return values
          },
        },
        location: { host: '124.223.157.37:3000', protocol: 'http:' },
        setTimeout,
      },
    })
    Object.defineProperty(globalThis, 'WebSocket', { configurable: true, value: FakeWebSocket })
    const room: Room = {
      id: 'room-1',
      name: 'HTTP 聊天室',
      has_password: false,
      creator_user_id: 'user-1',
      join_policy: 'open',
      avatar_emoji: '',
      description: '',
      membership_status: 'active',
      membership_role: 'member',
      unread_count: 0,
      created_at: '2026-08-22T00:00:00Z',
    }
    const warning = spyOn(console, 'warn').mockImplementation(() => {})
    const chat = useChatSocket()
    warning.mockRestore()
    chat.connect(room, 'session-token', 'user-1', '')
    const socket = FakeWebSocket.instances[0]
    socket?.onmessage?.({ data: JSON.stringify({ type: 'auth_ok', participants: [] }) } as MessageEvent<string>)

    expect(chat.send('HTTP 也能发送')).toBe(true)
    expect(chat.messages.value).toHaveLength(1)
    const payload = JSON.parse(socket?.sent.at(-1) || '{}') as { client_message_id?: string }
    expect(payload.client_message_id).toMatch(/^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/)
    chat.close()
  })

  test('does not queue or resend a pending message after reconnect', async () => {
    Object.defineProperty(globalThis, 'window', {
      configurable: true,
      value: {
        clearTimeout,
        crypto: globalThis.crypto,
        location: { host: 'localhost:3000', protocol: 'http:' },
        setTimeout,
      },
    })
    Object.defineProperty(globalThis, 'WebSocket', { configurable: true, value: FakeWebSocket })
    const room: Room = {
      id: 'room-1',
      name: '重连聊天室',
      has_password: false,
      creator_user_id: 'user-1',
      join_policy: 'open',
      avatar_emoji: '',
      description: '',
      membership_status: 'active',
      membership_role: 'member',
      unread_count: 0,
      created_at: '2026-08-27T00:00:00Z',
    }
    const warning = spyOn(console, 'warn').mockImplementation(() => {})
    const chat = useChatSocket()
    warning.mockRestore()
    chat.connect(room, 'session-token', 'user-1', '')
    const firstSocket = FakeWebSocket.instances[0]
    firstSocket?.onmessage?.({ data: JSON.stringify({ type: 'auth_ok', participants: [] }) } as MessageEvent<string>)

    expect(chat.send('只发送一次')).toBe(true)
    firstSocket?.onclose?.()
    expect((chat.messages.value[0] as BroadcastMessage).delivery_state).toBe('failed')

    await new Promise((resolve) => setTimeout(resolve, 550))
    const reconnectedSocket = FakeWebSocket.instances[1]
    reconnectedSocket?.onopen?.()
    reconnectedSocket?.onmessage?.({
      data: JSON.stringify({ type: 'auth_ok', participants: [] }),
    } as MessageEvent<string>)

    expect(reconnectedSocket?.sent.map((payload) => JSON.parse(payload).type)).toEqual(['join'])
    expect(chat.messages.value.filter((message) => message.type === 'broadcast')).toHaveLength(1)
    chat.close()
  })
})
