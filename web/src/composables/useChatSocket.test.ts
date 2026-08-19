import { afterEach, describe, expect, spyOn, test } from 'bun:test'
import { useChatSocket } from './useChatSocket'
import type { Room } from '../types'

class FakeWebSocket {
  static readonly OPEN = 1
  static instances: FakeWebSocket[] = []

  onopen: (() => void) | null = null
  onmessage: ((event: MessageEvent<string>) => void) | null = null
  onerror: (() => void) | null = null
  onclose: (() => void) | null = null
  readyState = FakeWebSocket.OPEN

  constructor(readonly url: string) {
    FakeWebSocket.instances.push(this)
  }

  close(): void {}
  send(): void {}
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
})
