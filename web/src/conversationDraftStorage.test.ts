import { describe, expect, test } from 'bun:test'
import { createConversationDraftStorage, resolveDraftReply } from './conversationDraftStorage'
import type { BroadcastMessage, DisplayMessage } from './types'

class MemoryStorage implements Storage {
  private readonly values = new Map<string, string>()

  get length(): number {
    return this.values.size
  }

  clear(): void {
    this.values.clear()
  }

  getItem(key: string): string | null {
    return this.values.get(key) ?? null
  }

  key(index: number): string | null {
    return [...this.values.keys()][index] ?? null
  }

  removeItem(key: string): void {
    this.values.delete(key)
  }

  setItem(key: string, value: string): void {
    this.values.set(key, value)
  }
}

function message(messageId: string, recalledAt: string | null = null): BroadcastMessage {
  return {
    type: 'broadcast',
    message_id: messageId,
    sender_id: 'user-2',
    sender: 'Lin',
    sender_avatar: 'L',
    content: 'source',
    attachment: null,
    reply_to: null,
    recalled_at: recalledAt,
    edited_at: null,
    timestamp: '2026-08-27T00:00:00Z',
    forwarded_from: null,
    reactions: [],
  }
}

describe('conversation draft storage', () => {
  test('isolates text and reply targets by account and room', () => {
    const storage = createConversationDraftStorage(new MemoryStorage(), () => '2026-08-27T01:02:03Z')

    storage.write('user-1', 'room-1', 'first', 'message-1')
    storage.write('user-1', 'room-2', 'second', null)

    expect(storage.read('user-1', 'room-1')).toEqual({
      content: 'first',
      reply_to_message_id: 'message-1',
      updated_at: '2026-08-27T01:02:03Z',
    })
    expect(storage.read('user-1', 'room-2')?.content).toBe('second')
    expect(storage.read('user-2', 'room-1')).toBeNull()
  })

  test('removes an empty draft instead of retaining account data', () => {
    const storage = createConversationDraftStorage(new MemoryStorage())
    storage.write('user-1', 'room-1', 'temporary', null)
    storage.write('user-1', 'room-1', '', null)
    expect(storage.read('user-1', 'room-1')).toBeNull()
  })

  test('degrades missing and recalled reply targets to no reply', () => {
    const active = message('active')
    const recalled = message('recalled', '2026-08-27T00:10:00Z')
    const messages: DisplayMessage[] = [active, recalled]

    expect(resolveDraftReply(messages, 'active')).toBe(active)
    expect(resolveDraftReply(messages, 'recalled')).toBeNull()
    expect(resolveDraftReply(messages, 'deleted')).toBeNull()
  })

  test('keeps typing usable when browser storage is unavailable', () => {
    const unavailable = {
      getItem: () => {
        throw new Error('blocked')
      },
      setItem: () => {
        throw new Error('blocked')
      },
      removeItem: () => {
        throw new Error('blocked')
      },
    } as unknown as Storage
    const storage = createConversationDraftStorage(unavailable)

    expect(() => storage.write('user-1', 'room-1', 'still editable', null)).not.toThrow()
    expect(storage.read('user-1', 'room-1')).toBeNull()
  })
})
