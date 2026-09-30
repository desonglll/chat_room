// TG-107 additions to presenceStore: stable indicator order and account-wide user statuses.
import { expect, test } from 'bun:test'
import { createPresenceStore, selectPresence, selectUserStatus } from './presenceStore'

test('a refresh frame updates an indicator in place, keeping first-seen order', () => {
  const store = createPresenceStore()
  const typing = (user_id: string, action: 'typing' | 'recording_voice', at: number) =>
    store.getState().applyTyping('c1', { type: 'typing', content: 'x', action, user_id, username: user_id }, at)
  typing('a', 'typing', 1)
  typing('b', 'typing', 2)
  typing('a', 'recording_voice', 3)
  const indicators = selectPresence('c1')(store.getState()).typing
  expect(indicators.map((indicator) => [indicator.user_id, indicator.action, indicator.receivedAt])).toEqual([
    ['a', 'recording_voice', 3],
    ['b', 'typing', 2],
  ])
})

test('statuses from auth_ok and user_status are also kept per user across chats', () => {
  const store = createPresenceStore()
  store.getState().applyAuthOk('c1', {
    type: 'auth_ok',
    room_name: 'x',
    members: [],
    participants: [],
    read_receipts: [],
    statuses: [{ user_id: 'u1', status: { kind: 'online' } }],
  })
  expect(selectUserStatus('u1')(store.getState())).toEqual({ kind: 'online' })
  store.getState().applyUserStatus('c2', { type: 'user_status', user_id: 'u1', status: { kind: 'recently' } })
  expect(selectUserStatus('u1')(store.getState())).toEqual({ kind: 'recently' })
  expect(selectUserStatus('nobody')(store.getState())).toBeUndefined()
})
