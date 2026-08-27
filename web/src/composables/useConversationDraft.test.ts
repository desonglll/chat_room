import { describe, expect, test } from 'bun:test'
import { effectScope, nextTick, reactive, ref } from 'vue'
import { createConversationDraftStorage } from '../conversationDraftStorage'
import { useConversationDraft } from './useConversationDraft'
import type { BroadcastMessage } from '../types'

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

function message(id: string, content = 'source'): BroadcastMessage {
  return {
    type: 'broadcast',
    message_id: id,
    sender_id: 'user-2',
    sender: 'Lin',
    sender_avatar: 'L',
    content,
    attachment: null,
    reply_to: null,
    recalled_at: null,
    edited_at: null,
    timestamp: '2026-08-27T00:00:00Z',
    forwarded_from: null,
    reactions: [],
  }
}

describe('conversation draft lifecycle', () => {
  test('restores only after history is ready and persists later input', async () => {
    const storage = createConversationDraftStorage(new MemoryStorage(), () => '2026-08-27T01:00:00Z')
    const source = message('message-1')
    storage.write('user-1', 'room-1', 'saved', source.message_id)
    const props = reactive({
      draftContext: { userId: 'user-1', ready: false },
      roomId: 'room-1',
      messages: [source],
      replyingTo: null as BroadcastMessage | null,
      editingTo: null as BroadcastMessage | null,
    })
    const draft = ref('')
    const scope = effectScope()
    scope.run(() =>
      useConversationDraft(
        props,
        {
          draft,
          updateReply: (reply) => (props.replyingTo = reply),
          editingLoaded: () => {},
        },
        storage,
      ),
    )

    expect(draft.value).toBe('')
    props.draftContext.ready = true
    await nextTick()
    expect(draft.value).toBe('saved')
    expect(props.replyingTo?.message_id).toBe(source.message_id)

    draft.value = 'changed'
    await nextTick()
    expect(storage.read('user-1', 'room-1')?.content).toBe('changed')
    scope.stop()
  })

  test('keeps the saved draft while editing an existing message', async () => {
    const storage = createConversationDraftStorage(new MemoryStorage())
    storage.write('user-1', 'room-1', 'original draft', null)
    const props = reactive({
      draftContext: { userId: 'user-1', ready: true },
      roomId: 'room-1',
      messages: [] as BroadcastMessage[],
      replyingTo: null as BroadcastMessage | null,
      editingTo: null as BroadcastMessage | null,
    })
    const draft = ref('')
    const scope = effectScope()
    scope.run(() =>
      useConversationDraft(
        props,
        {
          draft,
          updateReply: (reply) => (props.replyingTo = reply),
          editingLoaded: () => {},
        },
        storage,
      ),
    )

    expect(draft.value).toBe('original draft')
    props.editingTo = message('message-2', 'existing message')
    await nextTick()
    expect(draft.value).toBe('existing message')
    draft.value = 'edited message'
    await nextTick()
    expect(storage.read('user-1', 'room-1')?.content).toBe('original draft')

    props.editingTo = null
    await nextTick()
    expect(draft.value).toBe('original draft')
    scope.stop()
  })
})
