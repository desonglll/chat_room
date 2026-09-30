/** Pure chat-list logic: Telegram time rules, preview text, order, folders, row model. */
import { describe, expect, test } from 'bun:test'
import type { ConversationSummary } from '@tg/core'
import { isConversationMuted, sortConversations } from '@tg/core'
import { selectChatListView } from './chatListFilters'
import { buildChatPreview, mediaFromFileName } from './chatPreview'
import { toChatRowModel } from './chatRowModel'
import { deriveChatRowPresence } from './chatRowPresence'
import { formatChatListTime } from './chatTime'
import { conversation } from './chatListFixtures'

// Local-time fixtures: the formatter is local-time by design, so build dates locally.
const at = (y: number, m: number, d: number, h = 12, min = 0) => new Date(y, m - 1, d, h, min)
const NOW = at(2026, 10, 1, 15, 30) // Thursday

const message = (overrides: Partial<NonNullable<ConversationSummary['last_message']>> = {}) => ({
  message_id: 'm1',
  sender_id: 'u2',
  sender: 'alice',
  content: 'hello',
  attachment_file_name: null,
  recalled: false,
  created_at: at(2026, 10, 1, 9, 5).toISOString(),
  ...overrides,
})

describe('formatChatListTime', () => {
  test('today is HH:mm, zero-padded', () => {
    expect(formatChatListTime(at(2026, 10, 1, 9, 5).toISOString(), NOW)).toBe('09:05')
    expect(formatChatListTime(at(2026, 10, 1, 0, 0).toISOString(), NOW)).toBe('00:00')
  })
  test('yesterday through six days ago is the weekday', () => {
    expect(formatChatListTime(at(2026, 9, 30, 23, 59).toISOString(), NOW)).toBe('周三')
    expect(formatChatListTime(at(2026, 9, 25).toISOString(), NOW)).toBe('周五')
  })
  test('a week or more ago is a date; the year appears only for another year', () => {
    expect(formatChatListTime(at(2026, 9, 24).toISOString(), NOW)).toBe('9月24日')
    expect(formatChatListTime(at(2025, 12, 31).toISOString(), NOW)).toBe('2025/12/31')
  })
  test('small clock skew into the future still reads as a time; garbage is empty', () => {
    expect(formatChatListTime(at(2026, 10, 1, 15, 31).toISOString(), NOW)).toBe('15:31')
    expect(formatChatListTime('not a date', NOW)).toBe('')
  })
})

describe('buildChatPreview', () => {
  const base = { chatType: 'group' as const, currentUserId: 'me', draftText: '' }

  test('group: other sender prefixed by name, own by 你, system unprefixed', () => {
    expect(buildChatPreview({ ...base, lastMessage: message() })).toEqual({
      kind: 'message',
      sender: 'alice',
      media: null,
      text: 'hello',
      recalled: false,
    })
    expect(buildChatPreview({ ...base, lastMessage: message({ sender_id: 'me' }) })).toMatchObject({ sender: '你' })
    expect(buildChatPreview({ ...base, lastMessage: message({ sender_id: null }) })).toMatchObject({ sender: null })
  })

  test('private chats and channels never prefix', () => {
    expect(buildChatPreview({ ...base, chatType: 'private', lastMessage: message() })).toMatchObject({ sender: null })
    expect(buildChatPreview({ ...base, chatType: 'channel', lastMessage: message() })).toMatchObject({ sender: null })
  })

  test('media: glyph kind from the file name, caption wins, else a label or the file name', () => {
    expect(
      buildChatPreview({ ...base, lastMessage: message({ content: '', attachment_file_name: 'a.JPG' }) }),
    ).toMatchObject({
      media: 'photo',
      text: '图片',
    })
    expect(
      buildChatPreview({ ...base, lastMessage: message({ content: 'look', attachment_file_name: 'clip.mp4' }) }),
    ).toMatchObject({ media: 'video', text: 'look' })
    expect(
      buildChatPreview({ ...base, lastMessage: message({ content: '', attachment_file_name: 'report.pdf' }) }),
    ).toMatchObject({ media: 'file', text: 'report.pdf' })
    expect(mediaFromFileName('voice.ogg')).toBe('voice')
    expect(mediaFromFileName('anim.gif')).toBe('gif')
    expect(mediaFromFileName('noext')).toBe('file')
    expect(mediaFromFileName(null)).toBeNull()
  })

  test('multi-line content collapses to one line', () => {
    expect(buildChatPreview({ ...base, lastMessage: message({ content: 'a\n\n  b\tc' }) })).toMatchObject({
      text: 'a b c',
    })
  })

  test('recalled message', () => {
    expect(buildChatPreview({ ...base, lastMessage: message({ recalled: true }) })).toMatchObject({
      text: '消息已撤回',
      recalled: true,
      sender: null,
    })
  })

  test('priority: typing > draft > message; the open chat shows no draft marker', () => {
    const lastMessage = message()
    expect(buildChatPreview({ ...base, lastMessage, draftText: 'wip', typingText: 'bob 正在输入…' })).toEqual({
      kind: 'typing',
      text: 'bob 正在输入…',
    })
    expect(buildChatPreview({ ...base, lastMessage, draftText: ' wip\n2 ' })).toEqual({ kind: 'draft', text: 'wip 2' })
    expect(buildChatPreview({ ...base, lastMessage, draftText: 'wip', active: true }).kind).toBe('message')
    expect(buildChatPreview({ ...base, lastMessage, draftText: '   ' }).kind).toBe('message')
  })

  test('empty chat hint differs for private and group', () => {
    expect(buildChatPreview({ ...base, lastMessage: null })).toEqual({ kind: 'empty', text: '暂无消息' })
    expect(buildChatPreview({ ...base, chatType: 'private', lastMessage: null })).toEqual({
      kind: 'empty',
      text: '开始聊天',
    })
  })
})

describe('sortConversations', () => {
  test('pinned first, then newest activity, archived last, id tie-break', () => {
    const rows = [
      conversation('old', { last_activity_at: '2026-09-01T00:00:00Z' }),
      conversation('new', { last_activity_at: '2026-09-30T00:00:00Z' }),
      conversation('pin', {
        last_activity_at: '2026-08-01T00:00:00Z',
        preferences: { ...conversation('pin').preferences, is_pinned: true },
      }),
      conversation('arch', {
        last_activity_at: '2026-10-01T00:00:00Z',
        preferences: { ...conversation('arch').preferences, is_archived: true, is_pinned: true },
      }),
      conversation('b', { last_activity_at: '2026-09-15T00:00:00Z' }),
      conversation('a', { last_activity_at: '2026-09-15T00:00:00Z' }),
    ]
    expect(sortConversations(rows).map((row) => row.room_id)).toEqual(['pin', 'new', 'a', 'b', 'old', 'arch'])
  })

  test('a timestamp with an offset compares by instant, not by string', () => {
    const rows = [
      conversation('utc', { last_activity_at: '2026-09-30T10:00:00Z' }),
      conversation('offset', { last_activity_at: '2026-09-30T12:00:00+08:00' }), // 04:00Z
    ]
    expect(sortConversations(rows).map((row) => row.room_id)).toEqual(['utc', 'offset'])
  })
})

describe('muted', () => {
  test('notification level none, or muted_until in the future', () => {
    const now = Date.parse('2026-10-01T00:00:00Z')
    const prefs = conversation('x').preferences
    expect(isConversationMuted(conversation('x'), now)).toBe(false)
    expect(isConversationMuted(conversation('x', { preferences: { ...prefs, notification_level: 'none' } }), now)).toBe(
      true,
    )
    expect(
      isConversationMuted(conversation('x', { preferences: { ...prefs, muted_until: '2026-10-02T00:00:00Z' } }), now),
    ).toBe(true)
    expect(
      isConversationMuted(conversation('x', { preferences: { ...prefs, muted_until: '2026-09-30T00:00:00Z' } }), now),
    ).toBe(false)
  })
})

describe('selectChatListView', () => {
  const archived = (id: string, unread: number) =>
    conversation(id, {
      unread_count: unread,
      preferences: { ...conversation(id).preferences, is_archived: true },
    })
  const rows = [
    conversation('a', { title: 'Alpha' }),
    conversation('b', { title: 'Beta' }),
    archived('z', 3),
    archived('y', 2),
  ]

  test('main folder hides archived rows and summarises them for the entry row', () => {
    const view = selectChatListView(rows, 'main', '')
    expect(view.rows.map((row) => row.room_id).sort()).toEqual(['a', 'b'])
    expect(view.archivedCount).toBe(2)
    expect(view.archivedUnread).toBe(5)
  })

  test('archive folder shows only archived rows', () => {
    expect(
      selectChatListView(rows, 'archive', '')
        .rows.map((row) => row.room_id)
        .sort(),
    ).toEqual(['y', 'z'])
  })

  test('search matches title, alias and peer names across both folders, case-insensitively', () => {
    expect(selectChatListView(rows, 'main', 'alp').rows.map((row) => row.room_id)).toEqual(['a'])
    const direct = conversation('d', {
      kind: 'direct',
      title: 'Zed',
      peer: { id: 'u9', username: 'zed_user', avatar_emoji: '', display_name: 'Zedd' },
    })
    expect(selectChatListView([...rows, direct], 'main', 'ZED_U').rows.map((row) => row.room_id)).toEqual(['d'])
    expect(selectChatListView(rows, 'main', 'chat z').rows.map((row) => row.room_id)).toEqual(['z'])
  })
})

describe('toChatRowModel', () => {
  const context = { currentUserId: 'me', activeChatId: '', draftText: '', now: NOW }

  test('own last message gets a sent tick; others none; read via the seam', () => {
    const own = conversation('c', { last_message: message({ sender_id: 'me' }) })
    expect(toChatRowModel(own, context).outgoing).toBe('sent')
    expect(toChatRowModel(own, { ...context, isReadByPeer: () => true }).outgoing).toBe('read')
    expect(toChatRowModel(conversation('c', { last_message: message() }), context).outgoing).toBeNull()
  })

  test('unread is suppressed for the open chat; pin is ignored inside the archive', () => {
    const row = conversation('c', {
      unread_count: 4,
      preferences: { ...conversation('c').preferences, is_pinned: true },
    })
    expect(toChatRowModel(row, context)).toMatchObject({ unreadCount: 4, pinned: true })
    expect(toChatRowModel(row, { ...context, activeChatId: 'c' }).unreadCount).toBe(0)
    const archivedPinned = conversation('c', {
      preferences: { ...conversation('c').preferences, is_pinned: true, is_archived: true },
    })
    expect(toChatRowModel(archivedPinned, context).pinned).toBe(false)
  })

  test('alias wins over title; an /api/ avatar is an image, anything else initials', () => {
    const row = conversation('c', { alias: 'Mom', avatar_emoji: '/api/users/u/avatar' })
    expect(toChatRowModel(row, context)).toMatchObject({
      title: 'Mom',
      avatarSrc: '/api/users/u/avatar',
      avatarInitials: undefined,
    })
    expect(toChatRowModel(conversation('c', { avatar_emoji: '🐱' }), context).avatarInitials).toBe('🐱')
  })

  test('private chats show presence; time falls back to last activity', () => {
    const direct = conversation('d', { kind: 'direct', last_activity_at: at(2026, 10, 1, 8, 0).toISOString() })
    const model = toChatRowModel(direct, context)
    expect(model).toMatchObject({ chatType: 'private', showsPresence: true, time: '08:00' })
    expect(toChatRowModel(conversation('g'), context).showsPresence).toBe(false)
  })
})

describe('deriveChatRowPresence (TG-107 seam)', () => {
  const presence = {
    members: [],
    participants: [{ user_id: 'u9', username: 'zed', avatar_emoji: '' }],
    typing: [] as Array<{ user_id: string; username: string; content: string; action: 'typing'; receivedAt: number }>,
    statuses: {},
  }
  const direct = conversation('d', {
    kind: 'direct',
    peer: { id: 'u9', username: 'zed', avatar_emoji: '', display_name: '' },
  })

  test('unknown without presence data', () => {
    expect(deriveChatRowPresence(undefined, direct, 'me')).toEqual({ isOnline: undefined, typingText: null })
  })

  test('private: peer online; typing text without a name', () => {
    const typing = [{ user_id: 'u9', username: 'zed', content: '', action: 'typing' as const, receivedAt: 0 }]
    expect(deriveChatRowPresence({ ...presence, typing }, direct, 'me')).toEqual({
      isOnline: true,
      typingText: '正在输入…',
    })
  })

  test('group: named typist, count for several, never the caller', () => {
    const typist = (id: string, name: string) => ({
      user_id: id,
      username: name,
      content: '',
      action: 'typing' as const,
      receivedAt: 0,
    })
    const group = conversation('g')
    expect(deriveChatRowPresence({ ...presence, typing: [typist('a', 'amy')] }, group, 'me').typingText).toBe(
      'amy 正在输入…',
    )
    expect(
      deriveChatRowPresence({ ...presence, typing: [typist('a', 'amy'), typist('b', 'bo')] }, group, 'me').typingText,
    ).toBe('2 人正在输入…')
    expect(deriveChatRowPresence({ ...presence, typing: [typist('me', 'me')] }, group, 'me').typingText).toBeNull()
  })
})
