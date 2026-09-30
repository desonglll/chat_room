/** The notification switch maps onto the existing conversation-preference API. */
import { expect, test } from 'bun:test'
import { notificationsEnabled, notificationsPatch, updateConversationPreferences } from '../chatInfoApi'
import { directConversation, fakeClient } from './fixtures'

const preferences = directConversation().preferences
const NOW = Date.parse('2026-10-01T00:00:00Z')

test('switch state: level none or a future timed mute is off', () => {
  expect(notificationsEnabled(null, NOW)).toBe(true)
  expect(notificationsEnabled(preferences, NOW)).toBe(true)
  expect(notificationsEnabled({ ...preferences, notification_level: 'none' }, NOW)).toBe(false)
  expect(notificationsEnabled({ ...preferences, muted_until: '2026-10-02T00:00:00Z' }, NOW)).toBe(false)
  expect(notificationsEnabled({ ...preferences, muted_until: '2026-09-30T00:00:00Z' }, NOW)).toBe(true)
})

test('switch on clears any timed mute; off mutes indefinitely; PATCHes the preference route', async () => {
  expect(notificationsPatch(true)).toEqual({ notification_level: 'all', muted_until: null })
  expect(notificationsPatch(false)).toEqual({ notification_level: 'none' })
  const { client, calls } = fakeClient(() => preferences)
  await updateConversationPreferences(client, 't', 'd1', notificationsPatch(false))
  expect(calls[0]).toMatchObject({ method: 'PATCH', path: '/api/conversations/d1/preferences' })
})
