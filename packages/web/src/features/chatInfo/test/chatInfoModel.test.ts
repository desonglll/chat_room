/** Header variant selection is driven by chat_type alone. */
import { expect, test } from 'bun:test'
import { createPresenceStore } from '@tg/core'
import {
  countMembers,
  infoVariant,
  memberCountText,
  selectInfoHeader,
  subscriberCountText,
  withKnownMembers,
} from '../chatInfoModel'
import { chat, directConversation, user } from './fixtures'

test('chat_type → variant: private, group, supergroup (a group), channel', () => {
  expect(infoVariant('private')).toBe('private')
  expect(infoVariant('group')).toBe('group')
  expect(infoVariant('supergroup')).toBe('group')
  expect(infoVariant('channel')).toBe('channel')
})

test('group header: title, member count, description, public username', () => {
  const header = selectInfoHeader({
    chatId: 'c1',
    chat: chat({ username: null }),
    conversation: null,
    peerProfile: null,
  })
  expect(header).toEqual({
    variant: 'group',
    title: '设计评审',
    avatarEmoji: '🎨',
    username: '',
    description: '每周三评审',
    memberCount: 6,
  })
})

test('channel header: subscriber count instead of members', () => {
  const header = selectInfoHeader({
    chatId: 'c1',
    chat: chat({ chat_type: 'channel', member_count: 12840, username: 'news' }),
    conversation: null,
    peerProfile: null,
  })
  expect(header).toMatchObject({ variant: 'channel', subscriberCount: 12840, username: 'news' })
  expect(header).not.toHaveProperty('memberCount')
  expect(subscriberCountText(12840)).toBe('12,840 位订阅者')
})

test('private header: the peer from the sidebar row, bio once the profile loads', () => {
  const conversation = directConversation()
  const before = selectInfoHeader({ chatId: 'd1', chat: null, conversation, peerProfile: null })
  expect(before).toEqual({
    variant: 'private',
    userId: 'u2',
    title: 'Bob 王',
    avatarEmoji: '🦊',
    username: 'bob',
    bio: '',
  })
  const after = selectInfoHeader({ chatId: 'd1', chat: null, conversation, peerProfile: user() })
  expect(after).toMatchObject({ variant: 'private', bio: '产品设计师' })
  // A user-set alias wins over the display name, as in the sidebar.
  expect(
    selectInfoHeader({ chatId: 'd1', chat: null, conversation: { ...conversation, alias: '老王' }, peerProfile: null })
      ?.title,
  ).toBe('老王')
})

test('private header without a sidebar row falls back to the presence peer id', () => {
  const header = selectInfoHeader({
    chatId: 'd1',
    chat: chat({ chat_type: 'private', title: '' }),
    conversation: null,
    peerProfile: null,
    peerIdFallback: 'u7',
  })
  expect(header).toMatchObject({ variant: 'private', userId: 'u7' })
})

test('unknown chat → no header', () => {
  expect(selectInfoHeader({ chatId: 'x', chat: null, conversation: null, peerProfile: null })).toBeNull()
})

test('member count never undercounts the members already known live', () => {
  const header = selectInfoHeader({
    chatId: 'c1',
    chat: chat({ member_count: 1 }),
    conversation: null,
    peerProfile: null,
  })
  expect(withKnownMembers(header, 6)).toMatchObject({ memberCount: 6 })
  expect(withKnownMembers(header, 0)).toBe(header)
  expect(memberCountText(6, 1)).toBe('6 位成员')
  expect(memberCountText(1200, 3)).toBe('1,200 位成员，3 人在线')
})

test('countMembers counts online by user status', () => {
  const presence = createPresenceStore()
  presence.setState({
    chats: {
      c1: {
        members: [
          { user_id: 'a', username: 'a', avatar_emoji: '' },
          { user_id: 'b', username: 'b', avatar_emoji: '' },
        ],
        participants: [],
        typing: [],
        statuses: {},
      },
    },
    users: { a: { kind: 'online' }, b: { kind: 'recently' } },
  })
  expect(countMembers(presence.getState(), 'c1')).toEqual({ members: 2, online: 1 })
})
