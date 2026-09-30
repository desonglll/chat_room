// TG-011: the nine vanilla store skeletons. Each store is created fresh via its factory
// (the singletons are the same closures), so tests stay isolated without React anywhere.
import { describe, expect, test } from 'bun:test'
import type { Chat, CoreStorage, DraftUpdatedFrame } from '../types'
import { createAuthStore, selectToken } from './authStore'
import { createChatListStore, selectChatById } from './chatListStore'
import { createComposerStore, EMPTY_DRAFT, selectDraft } from './composerStore'
import { createMediaStore, selectUploadPercent } from './mediaStore'
import { createMessageStore, selectTimeline } from './messageStore'
import { createPresenceStore, selectPresence, TYPING_TTL_MS } from './presenceStore'
import { createSettingsStore, SETTINGS_STORAGE_KEY } from './settingsStore'
import { createStickerStore } from './stickerStore'
import { createUiStore, selectTopModal } from './uiStore'

const chat = (id: string, title: string): Chat => ({
  id,
  chat_type: 'group',
  title,
  has_password: false,
  creator_user_id: null,
  join_policy: 'open',
  avatar_emoji: '',
  description: '',
  username: null,
  is_forum: false,
  linked_chat_id: null,
  slow_mode_seconds: 0,
  auto_delete_seconds: 0,
  signatures_enabled: false,
  history_visible_to_new_members: true,
  member_count: 1,
  unread_count: 0,
  created_at: '2026-09-30T12:00:00Z',
})

describe('authStore', () => {
  test('tracks the session lifecycle and exposes the token selector', () => {
    const store = createAuthStore()
    expect(selectToken(store.getState())).toBe('')
    store.getState().setAuthenticating()
    expect(store.getState().status).toBe('authenticating')
    store.getState().setSession({
      token: 'tok',
      user: {
        id: 'u',
        username: 'alice',
        avatar_emoji: '',
        display_name: '',
        signature: '',
        homepage: '',
        created_at: '',
      },
      expires_at: '2027-01-01T00:00:00Z',
    })
    expect(selectToken(store.getState())).toBe('tok')
    store.getState().clearSession()
    expect(store.getState().status).toBe('anonymous')
  })
})

describe('chatListStore', () => {
  test('upserts, applies chat_updated descriptors and tracks unread counts', () => {
    const store = createChatListStore()
    store.getState().setChats([chat('c1', 'one')])
    store.getState().upsertChat(chat('c2', 'two'))
    store.getState().applyChatUpdated({ ...chat('c1', 'renamed'), member_count: 5 })
    store.getState().setUnreadCount('c2', 9)
    expect(selectChatById('c1')(store.getState())?.title).toBe('renamed')
    expect(selectChatById('c1')(store.getState())?.member_count).toBe(5)
    expect(selectChatById('c2')(store.getState())?.unread_count).toBe(9)
    store.getState().removeChat('c1')
    expect(selectChatById('c1')(store.getState())).toBeNull()
  })
})

describe('messageStore', () => {
  test('runs the optimistic send → broadcast reconciliation through the domain logic', () => {
    const store = createMessageStore()
    store.getState().appendOptimistic('c1', {
      clientMessageId: 'local-1',
      content: 'hi',
      replyTo: '',
      currentUserId: 'u1',
      participants: [],
    })
    expect(selectTimeline('c1')(store.getState()).messages).toHaveLength(1)

    const acknowledged = store.getState().applyBroadcast(
      'c1',
      {
        type: 'broadcast',
        message_id: 'server-1',
        client_message_id: 'local-1',
        sender_id: 'u1',
        sender: 'alice',
        sender_avatar: '',
        content: 'hi',
        attachment: null,
        reply_to: null,
        recalled_at: null,
        edited_at: null,
        timestamp: '2026-09-30T12:00:00Z',
        favorite_id: null,
        forwarded_from: null,
        reactions: [],
      },
      'none',
    )
    expect(acknowledged).toBe('local-1')
    const [message] = selectTimeline('c1')(store.getState()).messages
    expect(message).toMatchObject({ message_id: 'server-1', delivery_state: 'sent' })

    store.getState().applyRecall('c1', { type: 'message_recalled', message_id: 'server-1', recalled_at: 'now' })
    expect(selectTimeline('c1')(store.getState()).messages[0]).toMatchObject({ recalled_at: 'now' })
    store.getState().clearChat('c1')
    expect(selectTimeline('c1')(store.getState()).messages).toEqual([])
  })
})

describe('composerStore', () => {
  test('applies the frozen draft_updated frame verbatim and ignores stale syncs', () => {
    const store = createComposerStore()
    const frame: DraftUpdatedFrame = {
      type: 'draft_updated',
      user_id: 'u1',
      text: 'unsent thought',
      reply_to_message_id: 'm1',
      topic_id: null,
      updated_at: '2026-09-30T12:00:05Z',
    }
    store.getState().applyDraftUpdated('c1', frame)
    expect(selectDraft('c1')(store.getState())).toEqual({
      text: 'unsent thought',
      replyToMessageId: 'm1',
      topicId: null,
      updatedAt: '2026-09-30T12:00:05Z',
    })
    store.getState().applyDraftUpdated('c1', { ...frame, text: 'older', updated_at: '2026-09-30T12:00:01Z' })
    expect(selectDraft('c1')(store.getState()).text).toBe('unsent thought')
    store.getState().clearDraft('c1')
    expect(selectDraft('c1')(store.getState())).toEqual(EMPTY_DRAFT)
  })

  test('tracks local text, reply target and edit state per chat', () => {
    const store = createComposerStore()
    store.getState().setDraftText('c1', 'hello')
    store.getState().setReplyTarget('c1', 'm9')
    store.getState().startEditing('c2', 'm3')
    expect(selectDraft('c1')(store.getState())).toMatchObject({ text: 'hello', replyToMessageId: 'm9' })
    expect(store.getState().editing).toEqual({ c2: { messageId: 'm3', originalText: '', text: '' } })
    store.getState().stopEditing('c2')
    expect(store.getState().editing).toEqual({})
  })
})

describe('presenceStore', () => {
  test('applies typing actions with the cancel and legacy clear rules, plus expiry', () => {
    const store = createPresenceStore()
    store
      .getState()
      .applyTyping(
        'c1',
        { type: 'typing', content: 'dra', action: 'recording_voice', user_id: 'u2', username: 'bob' },
        1_000,
      )
    expect(selectPresence('c1')(store.getState()).typing).toHaveLength(1)

    // Legacy clear: empty content with plain typing.
    store.getState().applyTyping('c1', { type: 'typing', content: '', action: 'typing', user_id: 'u2' }, 2_000)
    expect(selectPresence('c1')(store.getState()).typing).toEqual([])

    store
      .getState()
      .applyTyping('c1', { type: 'typing', content: 'x', action: 'choosing_sticker', user_id: 'u3' }, 3_000)
    store.getState().applyTyping('c1', { type: 'typing', content: '', action: 'cancel', user_id: 'u3' }, 3_500)
    expect(selectPresence('c1')(store.getState()).typing).toEqual([])

    store.getState().applyTyping('c1', { type: 'typing', content: 'y', action: 'typing', user_id: 'u4' }, 4_000)
    store.getState().expireTyping('c1', 4_000 + TYPING_TTL_MS)
    expect(selectPresence('c1')(store.getState()).typing).toEqual([])
  })

  test('keeps the auth_ok status snapshot and applies user_status tiers over it', () => {
    const store = createPresenceStore()
    store.getState().applyAuthOk('c1', {
      type: 'auth_ok',
      room_name: 'general',
      members: [],
      participants: [],
      read_receipts: [],
      statuses: [{ user_id: 'u2', status: { kind: 'online' } }],
    })
    store.getState().applyUserStatus('c1', {
      type: 'user_status',
      user_id: 'u2',
      status: { kind: 'offline', last_seen: '2026-09-30T12:00:00Z' },
    })
    expect(selectPresence('c1')(store.getState()).statuses.u2).toEqual({
      kind: 'offline',
      last_seen: '2026-09-30T12:00:00Z',
    })
  })
})

describe('stickerStore', () => {
  test('keeps bounded recents and toggles favorites', () => {
    const store = createStickerStore()
    const ref = { set_id: 's1', sticker_id: 'a', emoji: '🦀' }
    store.getState().pushRecent(ref, 2)
    store.getState().pushRecent({ set_id: 's1', sticker_id: 'b', emoji: '🎉' }, 2)
    store.getState().pushRecent(ref, 2)
    expect(store.getState().recent.map((sticker) => sticker.sticker_id)).toEqual(['a', 'b'])
    store.getState().toggleFavorite(ref)
    store.getState().toggleFavorite(ref)
    expect(store.getState().favorites).toEqual([])
  })
})

describe('mediaStore', () => {
  test('drives upload percent through the migrated domain math', () => {
    const store = createMediaStore()
    store.getState().beginUpload({
      uploadId: 'up1',
      chatId: 'c1',
      fileName: 'cat.png',
      phase: 'queued',
      processedBytes: 0,
      totalBytes: 200,
    })
    expect(selectUploadPercent('up1')(store.getState())).toBe(0)
    store.getState().reportUploadProgress('up1', 'uploading', 100)
    expect(selectUploadPercent('up1')(store.getState())).toBe(50)
    store.getState().failUpload('up1', 'network')
    expect(store.getState().uploads.up1).toMatchObject({ status: 'failed', error: 'network' })
    store.getState().finishUpload('up1')
    expect(selectUploadPercent('up1')(store.getState())).toBe(0)
  })
})

describe('settingsStore', () => {
  test('hydrates from and persists to the injected storage only', () => {
    const backing = new Map<string, string>()
    const storage: CoreStorage = {
      getItem: (key) => backing.get(key) ?? null,
      setItem: (key, value) => void backing.set(key, value),
      removeItem: (key) => void backing.delete(key),
    }
    const store = createSettingsStore()
    store.getState().update({ theme: 'dark', sendShortcut: 'shift-enter' })
    store.getState().persist(storage)
    expect(JSON.parse(backing.get(SETTINGS_STORAGE_KEY)!)).toMatchObject({ theme: 'dark' })

    const second = createSettingsStore()
    second.getState().hydrate(storage)
    expect(second.getState().theme).toBe('dark')
    expect(second.getState().sendShortcut).toBe('shift-enter')

    backing.set(SETTINGS_STORAGE_KEY, 'not json')
    second.getState().hydrate(storage)
    expect(second.getState().theme).toBe('system')
  })
})

describe('uiStore', () => {
  test('tracks sidebar, panel and the modal stack', () => {
    const store = createUiStore()
    store.getState().setSidebarWidth(10)
    expect(store.getState().sidebarWidth).toBe(64) // clamped
    store.getState().setActiveChat('c1')
    store.getState().openPanel('chatInfo')
    store.getState().pushModal('confirm-delete')
    store.getState().pushModal('nested')
    expect(selectTopModal(store.getState())).toBe('nested')
    store.getState().popModal()
    expect(selectTopModal(store.getState())).toBe('confirm-delete')
    store.getState().closePanel()
    expect(store.getState()).toMatchObject({ activeChatId: 'c1', activePanel: 'none' })
  })
})
