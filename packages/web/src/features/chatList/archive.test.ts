/**
 * TG-502 archive logic: the unarchive-on-new-message matrix (muted × archived × sender), the
 * archive badge, the optimistic archive action with rollback, the account-socket wiring, the
 * persisted row mode and the swipe gesture's decisions.
 */
import { describe, expect, test } from 'bun:test'
import type {
  AccountMessageEvent,
  ConversationPreferences,
  ConversationSummary,
  CoreClock,
  CoreSocket,
  CoreStorage,
  FetchLike,
} from '@tg/core'
import { createApiClient, createChatListStore } from '@tg/core'
import { startAccountFeed } from './accountFeed'
import { applyUnarchiveRule, setChatArchived } from './archiveActions'
import { ARCHIVE_STORAGE_KEY, readArchiveRowMode, writeArchiveRowMode } from './archiveRowMode'
import { archiveBadge, archivePreviewChats, unarchivesOnMessage } from './archiveRules'
import { conversation } from './chatListFixtures'
import { commitsArchive, lockAxis, swipeOffset, swipeProgress } from './swipeGesture'

const NOW = Date.parse('2026-10-01T12:00:00Z')
const HOUR = 3_600_000

type Mute = 'none' | 'level-none' | 'level-mentions' | 'until-future' | 'until-past'

const MUTED: Record<Mute, boolean> = {
  none: false,
  'level-none': true,
  'level-mentions': false,
  'until-future': true,
  'until-past': false,
}

function preferences(mute: Mute, archived: boolean): Partial<ConversationPreferences> {
  return {
    is_archived: archived,
    notification_level: mute === 'level-none' ? 'none' : mute === 'level-mentions' ? 'mentions' : 'all',
    muted_until:
      mute === 'until-future'
        ? new Date(NOW + HOUR).toISOString()
        : mute === 'until-past'
          ? new Date(NOW - HOUR).toISOString()
          : null,
  }
}

const chat = (id: string, mute: Mute, archived: boolean, unread = 0): ConversationSummary => {
  const base = conversation(id, { unread_count: unread })
  return { ...base, preferences: { ...base.preferences, ...preferences(mute, archived) } }
}

const event = (roomId: string, senderId: string | null = 'u2'): AccountMessageEvent => ({
  type: 'new_message',
  message_id: `m-${roomId}`,
  room_id: roomId,
  conversation_kind: 'group',
  conversation_title: roomId,
  sender_id: senderId,
  sender: 'alice',
  content: 'hi',
  attachment_file_name: null,
  timestamp: '2026-10-01T12:00:00Z',
  is_mention: false,
})

const MUTES = Object.keys(MUTED) as Mute[]

describe('unarchive on a new message: muted × archived × new message', () => {
  for (const mute of MUTES) {
    for (const archived of [true, false]) {
      test(`${mute}, ${archived ? 'archived' : 'in the main list'}`, () => {
        const row = chat('a', mute, archived)
        // No message: nothing changes.
        const store = createChatListStore()
        store.getState().setConversations([row])
        expect(store.getState().conversations[0]?.preferences.is_archived).toBe(archived)
        // Another member's message and a system message: archived iff it was archived and muted.
        for (const sender of ['u2', null]) {
          expect(unarchivesOnMessage(row, sender, 'me', NOW)).toBe(archived && !MUTED[mute])
        }
        // Own message: never unarchives.
        expect(unarchivesOnMessage(row, 'me', 'me', NOW)).toBe(false)

        store.getState().applyAccountMessage(event('a'), '')
        applyUnarchiveRule(store, event('a'), 'me', NOW)
        expect(store.getState().conversations[0]?.preferences.is_archived).toBe(archived && MUTED[mute])
      })
    }
  }

  test('a message in another chat leaves this archive alone', () => {
    const store = createChatListStore()
    store.getState().setConversations([chat('a', 'none', true), chat('b', 'none', false)])
    applyUnarchiveRule(store, event('b'), 'me', NOW)
    expect(store.getState().conversations[0]?.preferences.is_archived).toBe(true)
  })

  test('the account socket applies the rule live', () => {
    const store = createChatListStore()
    store.getState().setConversations([chat('loud', 'none', true), chat('quiet', 'level-none', true)])
    let socket: (CoreSocket & { sent: string[] }) | null = null
    const clock: CoreClock = { now: () => NOW, setTimeout: () => 0, clearTimeout: () => {} }
    const stop = startAccountFeed({
      url: 'ws://x/ws/account',
      token: 'tok',
      clock,
      store,
      activeChatId: () => '',
      currentUserId: () => 'me',
      resync: () => {},
      socketFactory: () => {
        const created = {
          sent: [] as string[],
          send(data: string) {
            created.sent.push(data)
          },
          close() {},
          onopen: null as (() => void) | null,
          onmessage: null as ((data: string) => void) | null,
          onclose: null as ((info: { code: number; reason: string }) => void) | null,
          onerror: null as (() => void) | null,
        }
        socket = created
        return created
      },
    })
    const live = socket as unknown as CoreSocket
    live.onopen?.()
    live.onmessage?.(JSON.stringify(event('loud')))
    live.onmessage?.(JSON.stringify(event('quiet')))
    const byId = Object.fromEntries(store.getState().conversations.map((row) => [row.room_id, row]))
    expect(byId.loud?.preferences.is_archived).toBe(false)
    expect(byId.loud?.unread_count).toBe(1)
    expect(byId.quiet?.preferences.is_archived).toBe(true)
    expect(byId.quiet?.unread_count).toBe(1)
    stop()
  })
})

describe('archiveBadge', () => {
  test('sums unread over archived non-muted chats, accent', () => {
    const rows = [
      chat('a', 'none', true, 3),
      chat('b', 'until-past', true, 2),
      chat('c', 'level-none', true, 7),
      chat('d', 'none', false, 50),
    ]
    expect(archiveBadge(rows, NOW)).toEqual({ count: 5, muted: false })
  })

  test('grey with the muted sum when every archived unread chat is muted', () => {
    const rows = [chat('a', 'level-none', true, 4), chat('b', 'until-future', true, 1), chat('c', 'none', true, 0)]
    expect(archiveBadge(rows, NOW)).toEqual({ count: 5, muted: true })
  })

  test('no unread: no badge', () => {
    expect(archiveBadge([chat('a', 'none', true)], NOW).count).toBe(0)
  })
})

test('archivePreviewChats: archived only, unread first, capped', () => {
  const rows = [
    chat('a', 'none', true),
    chat('b', 'none', false, 3),
    chat('c', 'none', true, 1),
    chat('d', 'none', true),
  ]
  expect(archivePreviewChats(rows, 2).map((row) => row.room_id)).toEqual(['c', 'a'])
})

describe('setChatArchived', () => {
  function clientReplying(status: number) {
    const calls: Array<{ url: string; init?: RequestInit | undefined }> = []
    const fetchImpl: FetchLike = async (url, init) => {
      calls.push({ url, init })
      if (status !== 200) return new Response('no', { status })
      const body = JSON.parse(String(init?.body)) as { is_archived: boolean }
      return Response.json({ ...conversation('a').preferences, is_archived: body.is_archived, updated_at: 'now' })
    }
    return { client: createApiClient({ fetchImpl }), calls }
  }

  test('optimistic, then PATCHes the preference and keeps the server reply', async () => {
    const store = createChatListStore()
    store.getState().setConversations([conversation('a')])
    const { client, calls } = clientReplying(200)
    const pending = setChatArchived({ client, token: 'tok', store }, 'a', true)
    expect(store.getState().conversations[0]?.preferences.is_archived).toBe(true)
    expect(await pending).toBe(true)
    expect(calls[0]?.url).toBe('/api/conversations/a/preferences')
    expect(calls[0]?.init?.method).toBe('PATCH')
    expect(JSON.parse(String(calls[0]?.init?.body))).toEqual({ is_archived: true })
    expect(store.getState().conversations[0]?.preferences).toMatchObject({ is_archived: true, updated_at: 'now' })
  })

  test('rolls back when the server refuses', async () => {
    const store = createChatListStore()
    store.getState().setConversations([chat('a', 'none', true)])
    const { client } = clientReplying(404)
    expect(await setChatArchived({ client, token: 'tok', store }, 'a', false)).toBe(false)
    expect(store.getState().conversations[0]?.preferences.is_archived).toBe(true)
  })

  test('no request when nothing changes', async () => {
    const store = createChatListStore()
    store.getState().setConversations([conversation('a')])
    const { client, calls } = clientReplying(200)
    expect(await setChatArchived({ client, token: 'tok', store }, 'a', false)).toBe(true)
    expect(calls).toHaveLength(0)
  })
})

describe('archive row mode', () => {
  const memory = (): CoreStorage & { data: Map<string, string> } => {
    const data = new Map<string, string>()
    return {
      data,
      getItem: (key) => data.get(key) ?? null,
      setItem: (key, value) => void data.set(key, value),
      removeItem: (key) => void data.delete(key),
    }
  }

  test('defaults to collapsed, persists, ignores garbage', () => {
    const storage = memory()
    expect(readArchiveRowMode(storage)).toBe('collapsed')
    writeArchiveRowMode(storage, 'hidden')
    expect(readArchiveRowMode(storage)).toBe('hidden')
    storage.setItem(ARCHIVE_STORAGE_KEY, '{"mode":"sideways"}')
    expect(readArchiveRowMode(storage)).toBe('collapsed')
    storage.setItem(ARCHIVE_STORAGE_KEY, 'not json')
    expect(readArchiveRowMode(storage)).toBe('collapsed')
  })
})

describe('swipe gesture', () => {
  test('locks horizontal only for a leftward drag; vertical is a scroll', () => {
    expect(lockAxis(-4, 2)).toBe('pending')
    expect(lockAxis(-30, 5)).toBe('horizontal')
    expect(lockAxis(30, 5)).toBe('vertical')
    expect(lockAxis(-12, 20)).toBe('vertical')
  })

  test('offset never goes right, and rubber-bands past the row width', () => {
    expect(swipeOffset(40, 300)).toBe(0)
    expect(swipeOffset(-120, 300)).toBe(-120)
    const past = swipeOffset(-600, 300)
    expect(past).toBeLessThan(-300)
    expect(past).toBeGreaterThan(-600)
  })

  test('commits past a third of the width, or on a flick', () => {
    expect(commitsArchive(-90, 300, 0)).toBe(false)
    expect(commitsArchive(-110, 300, 0)).toBe(true)
    expect(commitsArchive(-40, 300, -1)).toBe(true)
    expect(commitsArchive(-20, 300, -1)).toBe(false)
    expect(swipeProgress(-105, 300)).toBe(1)
    expect(swipeProgress(0, 300)).toBe(0)
  })
})
