/**
 * TG-1208: the mute choices follow the chat's current state. The walkthrough found «取消静音»
 * (and every duration) offered on a chat that was not muted.
 */
import { expect, test } from 'bun:test'
import { isConversationMuted } from '@tg/core'
import { muteChoicesFor } from '../NotificationRow'
import { directConversation } from './fixtures'

const ids = (muted: boolean) => muteChoicesFor(muted).map((choice) => choice.id)

test('an unmuted chat offers only the durations; a muted one only «取消静音»', () => {
  expect(ids(false)).toEqual(['1h', '8h', '2d', 'forever'])
  expect(ids(true)).toEqual(['unmute'])
})

test('a timed mute that has lapsed counts as unmuted again', () => {
  const now = Date.parse('2026-10-01T12:00:00Z')
  const conversation = (patch: { notification_level?: 'all' | 'none'; muted_until?: string | null }) => {
    const base = directConversation()
    return { ...base, preferences: { ...base.preferences, ...patch } }
  }
  expect(ids(isConversationMuted(conversation({ muted_until: '2026-10-01T13:00:00Z' }), now))).toEqual(['unmute'])
  expect(ids(isConversationMuted(conversation({ muted_until: '2026-10-01T11:00:00Z' }), now))).not.toContain('unmute')
  expect(ids(isConversationMuted(conversation({ notification_level: 'none' }), now))).toEqual(['unmute'])
})
