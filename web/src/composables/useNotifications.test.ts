import { describe, expect, test } from 'bun:test'
import { mergeNotificationItems } from './useNotifications'
import type { NotificationItem } from '../notificationsApi'

const item = (id: string, summary: string): NotificationItem => ({
  id,
  kind: 'mention',
  actor: null,
  room_id: 'room-1',
  room_name: 'Room',
  message_id: id,
  run_id: null,
  summary,
  source_available: true,
  created_at: '2026-08-27T00:00:00Z',
  read_at: null,
})

describe('notification page merging', () => {
  test('keeps cursor pages stable and replaces refreshed duplicates', () => {
    expect(mergeNotificationItems([item('one', 'old')], [item('one', 'new'), item('two', 'next')])).toEqual([
      item('one', 'new'),
      item('two', 'next'),
    ])
  })
})
