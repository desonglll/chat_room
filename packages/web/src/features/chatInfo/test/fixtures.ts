/** Wire-shaped fixtures for the TG-106 tests. */
import type { Attachment, ApiClient, Chat, ChatFileItem, ChatMembership, ConversationSummary, User } from '@tg/core'

export function chat(overrides: Partial<Chat> = {}): Chat {
  return {
    id: 'c1',
    chat_type: 'group',
    title: '设计评审',
    has_password: false,
    creator_user_id: 'u1',
    join_policy: 'open',
    avatar_emoji: '🎨',
    description: '每周三评审',
    username: null,
    is_forum: false,
    linked_chat_id: null,
    slow_mode_seconds: 0,
    auto_delete_seconds: 0,
    signatures_enabled: false,
    history_visible_to_new_members: true,
    member_count: 6,
    unread_count: 0,
    created_at: '2026-09-01T00:00:00Z',
    ...overrides,
  }
}

export function directConversation(overrides: Partial<ConversationSummary> = {}): ConversationSummary {
  return {
    room_id: 'd1',
    kind: 'direct',
    title: 'bob',
    alias: '',
    avatar_emoji: '',
    description: '',
    group: null,
    peer: { id: 'u2', username: 'bob', avatar_emoji: '🦊', display_name: 'Bob 王' },
    unread_count: 0,
    pending_join_requests: 0,
    preferences: {
      room_id: 'd1',
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

export function user(overrides: Partial<User> = {}): User {
  return {
    id: 'u2',
    username: 'bob',
    avatar_emoji: '🦊',
    display_name: 'Bob 王',
    signature: '产品设计师',
    homepage: '',
    created_at: '2026-09-01T00:00:00Z',
    ...overrides,
  }
}

export function attachment(id: string, mime: string, name = `${id}.bin`): Attachment {
  return {
    id,
    file_name: name,
    mime_type: mime,
    size_bytes: 1024,
    download_url: `/api/attachments/${id}?key=k`,
    is_sensitive: false,
  }
}

export function fileItem(id: string, mime: string, createdAt = '2026-09-01T00:00:00Z'): ChatFileItem {
  return {
    message_id: `m-${id}`,
    sender_id: 'u1',
    sender: 'alice',
    sender_avatar: '',
    created_at: createdAt,
    attachment: attachment(id, mime),
  }
}

export function member(userId: string, role: ChatMembership['role'] = 'member', name = userId): ChatMembership {
  return {
    user_id: userId,
    username: name,
    avatar_emoji: '',
    nickname: '',
    role,
    status: 'active',
    requested_at: '2026-09-01T00:00:00Z',
    joined_at: '2026-09-01T00:00:00Z',
  }
}

export interface RecordedCall {
  method: string
  path: string
  query: Record<string, string>
}

/** An ApiClient that answers `json()` from a handler and records every call. */
export function fakeClient(answer: (call: RecordedCall) => unknown): { client: ApiClient; calls: RecordedCall[] } {
  const calls: RecordedCall[] = []
  const client: ApiClient = {
    request: () => Promise.reject(new Error('not used')),
    json: async <T>(method: string, path: string, options?: { query?: { toString(): string } }) => {
      const query = Object.fromEntries(
        (options?.query?.toString() ?? '')
          .split('&')
          .filter(Boolean)
          .map((pair) => pair.split('=').map(decodeURIComponent) as [string, string]),
      )
      const call = { method, path, query }
      calls.push(call)
      return answer(call) as T
    },
  }
  return { client, calls }
}
