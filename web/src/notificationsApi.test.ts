import { afterEach, describe, expect, mock, test } from 'bun:test'
import {
  getNotificationUnreadCount,
  listNotifications,
  markAllNotificationsRead,
  markNotificationRead,
  notificationParams,
} from './notificationsApi'

const originalFetch = globalThis.fetch

afterEach(() => {
  globalThis.fetch = originalFetch
})

describe('notifications API', () => {
  test('serializes kind, cursor, and page size without notification content', () => {
    const params = notificationParams('mention', '2026-08-27T00:00:00Z|mention:item', 20)
    expect(params.get('kind')).toBe('mention')
    expect(params.get('cursor')).toBe('2026-08-27T00:00:00Z|mention:item')
    expect(params.get('limit')).toBe('20')
  })

  test('uses recipient-scoped endpoints and safely encodes notification IDs', async () => {
    const fetchMock = mock(async (path: string) =>
      path.endsWith('unread-count')
        ? Response.json({ unread_count: 3 })
        : Response.json({ items: [], next_cursor: null }),
    )
    globalThis.fetch = fetchMock as typeof fetch

    await listNotifications('session-token', 'reply')
    expect(await getNotificationUnreadCount('session-token')).toBe(3)
    await markNotificationRead('session-token', 'reply:message/recipient')
    await markAllNotificationsRead('session-token')

    expect(String(fetchMock.mock.calls[0]![0])).toContain('/api/notifications?')
    expect(String(fetchMock.mock.calls[2]![0])).toContain('reply%3Amessage%2Frecipient/read')
    expect(fetchMock.mock.calls[3]![1]?.method).toBe('POST')
  })

  test('does not turn authorization failures into an empty inbox', async () => {
    globalThis.fetch = mock(async () => new Response(null, { status: 401 })) as typeof fetch
    expect(listNotifications('expired-token')).rejects.toThrow('登录已过期')
  })
})
