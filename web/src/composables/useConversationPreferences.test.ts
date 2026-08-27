import { describe, expect, it, mock } from 'bun:test'
import { ref } from 'vue'
import type { ConversationPreferences } from '../conversationPreferencesApi'
import type { ConversationSummary } from '../types'
import { useConversationPreferences } from './useConversationPreferences'

function conversation(roomId: string): ConversationSummary {
  return {
    room_id: roomId,
    kind: 'group',
    title: roomId,
    alias: '',
    avatar_emoji: '',
    description: '',
    group: null,
    peer: null,
    unread_count: 0,
    pending_join_requests: 0,
    preferences: {
      room_id: roomId,
      is_pinned: false,
      is_archived: false,
      notification_level: 'all',
      muted_until: null,
      updated_at: '2026-08-19T08:00:00Z',
    },
    last_message: null,
    last_activity_at: '2026-08-19T08:00:00Z',
    created_at: '2026-08-18T08:00:00Z',
  }
}

describe('conversation preference updates', () => {
  it('updates optimistically and replaces the projection with the server value', async () => {
    const items = ref([conversation('normal'), conversation('pinned')])
    let resolveWrite!: (value: ConversationPreferences) => void
    const write = mock(
      () =>
        new Promise<ConversationPreferences>((resolve) => {
          resolveWrite = resolve
        }),
    )
    const preferences = useConversationPreferences(items, ref('token'), write)
    const pending = preferences.update('pinned', { is_pinned: true })

    expect(items.value[0]?.room_id).toBe('pinned')
    expect(items.value[0]?.preferences.is_pinned).toBe(true)
    resolveWrite({
      ...items.value[0]!.preferences,
      notification_level: 'mentions',
      updated_at: '2026-08-20T08:00:00Z',
    })
    await pending

    expect(write).toHaveBeenCalledWith('pinned', { is_pinned: true }, 'token')
    expect(items.value[0]?.preferences.notification_level).toBe('mentions')
  })

  it('restores the previous preference and ordering when persistence fails', async () => {
    const items = ref([conversation('first'), conversation('second')])
    const write = mock(async () => {
      throw new Error('offline')
    })
    const preferences = useConversationPreferences(items, ref('token'), write)

    await expect(preferences.update('first', { is_archived: true })).rejects.toThrow('offline')
    expect(items.value.map((item) => item.room_id)).toEqual(['first', 'second'])
    expect(items.value[0]?.preferences.is_archived).toBe(false)
  })
})
