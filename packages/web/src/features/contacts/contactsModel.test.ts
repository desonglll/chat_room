/** TG-903: online first, then most recent exact last seen, then privacy buckets; filter in place. */
import { expect, test } from 'bun:test'
import type { SocialUser, UserStatusEntry } from '@tg/core'
import { contactRows } from './contactsModel'

const user = (id: string, username: string, display_name = '', remark = ''): SocialUser => ({
  id,
  username,
  avatar_emoji: '',
  display_name,
  signature: '',
  remark,
  relationship: 'friend',
})

const friends = [
  user('a', 'ann'),
  user('b', 'bob', 'Bob 王'),
  user('c', 'cat', '', '猫猫'),
  user('d', 'dan'),
  user('e', 'eve'),
]
const statuses: UserStatusEntry[] = [
  { user_id: 'a', status: { kind: 'offline', last_seen: '2026-10-01T08:00:00Z' } },
  { user_id: 'b', status: { kind: 'recently' } },
  { user_id: 'c', status: { kind: 'online' } },
  { user_id: 'd', status: { kind: 'offline', last_seen: '2026-10-01T09:00:00Z' } },
]

test('online first, newer exact last seen next, buckets after, unknown last', () => {
  expect(contactRows(friends, statuses, '').map((row) => row.user.id)).toEqual(['c', 'd', 'a', 'b', 'e'])
})

test('the filter matches remark, display name and @username, case-insensitively', () => {
  expect(contactRows(friends, statuses, '猫').map((row) => row.user.id)).toEqual(['c'])
  expect(contactRows(friends, statuses, 'bob 王').map((row) => row.user.id)).toEqual(['b'])
  expect(contactRows(friends, statuses, '@EV').map((row) => row.user.id)).toEqual(['e'])
  expect(contactRows(friends, statuses, 'zzz')).toEqual([])
})

test('the shown name prefers the remark', () => {
  expect(contactRows(friends, statuses, 'cat')[0]?.name).toBe('猫猫')
})
