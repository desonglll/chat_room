import { describe, expect, test } from 'bun:test'
import type { NotificationView } from './notifications'
import { notificationTarget } from './notifications'

const item = (extra: Partial<NotificationView>): NotificationView => ({
  id: 'n1',
  kind: 'mention',
  actor: null,
  room_id: 'c1',
  room_name: 'Team',
  message_id: 'm1',
  run_id: null,
  summary: '',
  source_available: true,
  created_at: '',
  read_at: null,
  ...extra,
})

describe('TG-703 notification targets', () => {
  test('a mention opens its message, a join request its chat, a friend request the contacts page', () => {
    expect(notificationTarget(item({}))).toBe('/chat/c1?message=m1')
    expect(notificationTarget(item({ kind: 'room_join_request', message_id: null }))).toBe('/chat/c1')
    expect(notificationTarget(item({ kind: 'friend_request', room_id: null }))).toBe('/contacts')
  })

  test('a source that is gone leads nowhere', () => {
    expect(notificationTarget(item({ source_available: false }))).toBeNull()
    expect(notificationTarget(item({ room_id: null }))).toBeNull()
  })
})
