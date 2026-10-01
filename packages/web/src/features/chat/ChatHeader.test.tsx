// TG-1206: a private chat's header shows the peer's emoji status after the name.
import { afterEach, beforeEach, expect, test } from 'bun:test'
import { renderToStaticMarkup } from 'react-dom/server'
import { MemoryRouter } from 'react-router-dom'
import type { ConversationSummary } from '@tg/core'
import { chatListStore } from '@tg/core'
import { ChatHeader } from './ChatHeader'
import { configureCustomEmoji } from '../customEmoji/services'
import { emojiFixture, installFakeServices } from '../customEmoji/test/fakeServices'

const row = (kind: 'direct' | 'group'): ConversationSummary =>
  ({
    room_id: 'c1',
    kind,
    title: 'Ann',
    alias: '',
    avatar_emoji: '',
    description: '',
    group: null,
    peer: kind === 'direct' ? { id: 'u-ann', username: 'ann', avatar_emoji: '', display_name: 'Ann' } : null,
    unread_count: 0,
    pending_join_requests: 0,
    preferences: { is_pinned: false, is_archived: false, is_muted: false },
    last_message: null,
    last_activity_at: '2026-10-01T10:00:00Z',
    created_at: '2026-10-01T10:00:00Z',
  }) as unknown as ConversationSummary

const header = () =>
  renderToStaticMarkup(
    <MemoryRouter>
      <ChatHeader chatId="c1" connection="online" />
    </MemoryRouter>,
  )

// The static renderer reads a store's server snapshot, i.e. `getInitialState`.
const initial = chatListStore.getInitialState
beforeEach(() => {
  chatListStore.getInitialState = chatListStore.getState
})

afterEach(() => {
  chatListStore.getInitialState = initial
  configureCustomEmoji(null)
  chatListStore.getState().setConversations([])
})

test('the peer status follows the title in a private chat, and nothing in a group', () => {
  const services = installFakeServices()
  services.statuses.seed([
    ['u-ann', { user_id: 'u-ann', custom_emoji_id: 'e1', expires_at: null, emoji: emojiFixture('e1', '⭐') }],
  ])
  services.emoji.seed([['e1', emojiFixture('e1', '⭐')]])

  chatListStore.getState().setConversations([row('direct')])
  let html = header()
  expect(html).toMatch(/tg-chat__title">Ann<[^]*tg-emoji-status/)

  chatListStore.getState().setConversations([row('group')])
  html = header()
  expect(html).not.toContain('tg-emoji-status')
})
