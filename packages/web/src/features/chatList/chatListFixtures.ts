/** Test-only fixtures for the chat-list tests (not imported by the app). */
import type { ConversationSummary } from '@tg/core'

export function conversation(id: string, overrides: Partial<ConversationSummary> = {}): ConversationSummary {
  return {
    room_id: id,
    kind: 'group',
    title: `Chat ${id}`,
    alias: '',
    avatar_emoji: '',
    description: '',
    group: null,
    peer: null,
    unread_count: 0,
    pending_join_requests: 0,
    preferences: {
      room_id: id,
      is_pinned: false,
      is_archived: false,
      notification_level: 'all',
      muted_until: null,
      updated_at: '2026-09-01T00:00:00Z',
    },
    last_message: null,
    last_activity_at: '2026-09-01T00:00:00Z',
    created_at: '2026-09-01T00:00:00Z',
    ...overrides,
  }
}
