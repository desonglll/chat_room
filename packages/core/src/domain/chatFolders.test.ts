import { describe, expect, test } from 'bun:test'
import type { ConversationSummary } from '../stores/chatListStore'
import type { ChatFolder } from './chatFolders'
import { folderConversations, folderUnread, inFolder } from './chatFolders'

const NOW = Date.parse('2026-10-01T12:00:00Z')

function chat(
  id: string,
  kind: 'direct' | 'group' | 'channel',
  extra: { unread?: number; muted?: boolean; archived?: boolean } = {},
): ConversationSummary {
  return {
    room_id: id,
    kind: kind === 'direct' ? 'direct' : 'group',
    title: id,
    alias: '',
    avatar_emoji: '',
    description: '',
    group:
      kind === 'direct'
        ? null
        : ({ chat_type: kind === 'channel' ? 'channel' : 'group' } as ConversationSummary['group']),
    peer: null,
    unread_count: extra.unread ?? 0,
    pending_join_requests: 0,
    preferences: {
      room_id: id,
      is_pinned: false,
      is_archived: extra.archived ?? false,
      notification_level: extra.muted ? 'none' : 'all',
      muted_until: null,
      updated_at: '',
    },
    last_message: null,
    last_activity_at: '',
    created_at: '',
  }
}

function folder(extra: Partial<ChatFolder>): ChatFolder {
  return {
    id: 'f',
    title: 'F',
    emoji: '',
    include_types: [],
    include_chat_ids: [],
    exclude_chat_ids: [],
    exclude_muted: false,
    exclude_read: false,
    exclude_archived: false,
    ...extra,
  }
}

describe('chat folder rules', () => {
  const dm = chat('dm', 'direct', { unread: 2 })
  const group = chat('group', 'group', { unread: 5 })
  const mutedGroup = chat('muted', 'group', { unread: 7, muted: true })
  const readGroup = chat('read', 'group')
  const archivedGroup = chat('archived', 'group', { unread: 1, archived: true })
  const channel = chat('channel', 'channel', { unread: 3 })
  const all = [dm, group, mutedGroup, readGroup, archivedGroup, channel]
  const ids = (f: ChatFolder) => folderConversations(f, all, NOW).map((c) => c.room_id)

  test('types take whole categories', () => {
    expect(ids(folder({ include_types: ['private'] }))).toEqual(['dm'])
    expect(ids(folder({ include_types: ['groups'] }))).toEqual(['group', 'muted', 'read', 'archived'])
    expect(ids(folder({ include_types: ['channels'] }))).toEqual(['channel'])
  })

  test('explicit include and exclude override the types', () => {
    expect(ids(folder({ include_types: ['groups'], exclude_chat_ids: ['group'] }))).toEqual([
      'muted',
      'read',
      'archived',
    ])
    expect(ids(folder({ include_chat_ids: ['channel'] }))).toEqual(['channel'])
    expect(ids(folder({ include_chat_ids: ['channel'], exclude_chat_ids: ['channel'] }))).toEqual([])
  })

  test('exclusion flags remove muted, read and archived chats, even explicitly included ones', () => {
    const groups = { include_types: ['groups'] as ChatFolder['include_types'] }
    expect(ids(folder({ ...groups, exclude_muted: true }))).toEqual(['group', 'read', 'archived'])
    expect(ids(folder({ ...groups, exclude_read: true }))).toEqual(['group', 'muted', 'archived'])
    expect(ids(folder({ ...groups, exclude_archived: true }))).toEqual(['group', 'muted', 'read'])
    expect(inFolder(folder({ include_chat_ids: ['read'], exclude_read: true }), readGroup, NOW)).toBe(false)
  })

  test('a timed mute counts as muted only until it lifts', () => {
    const timed = chat('timed', 'group')
    timed.preferences.muted_until = '2026-10-01T13:00:00Z'
    expect(inFolder(folder({ include_types: ['groups'], exclude_muted: true }), timed, NOW)).toBe(false)
    expect(
      inFolder(folder({ include_types: ['groups'], exclude_muted: true }), timed, Date.parse('2026-10-01T14:00:00Z')),
    ).toBe(true)
  })

  test("the folder's unread count is the sum of its chats' counts", () => {
    expect(folderUnread(folder({ include_types: ['groups'] }), all, NOW)).toBe(5 + 7 + 0 + 1)
    expect(folderUnread(folder({ include_types: ['private', 'channels'] }), all, NOW)).toBe(2 + 3)
    expect(folderUnread(folder({ include_types: ['groups'], exclude_muted: true }), all, NOW)).toBe(5 + 1)
  })
})
