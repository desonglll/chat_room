import { afterEach, describe, expect, mock, test } from 'bun:test'
import { listDeviceSessions, revokeDeviceSession, revokeOtherDeviceSessions } from './sessionApi'

const originalFetch = globalThis.fetch

afterEach(() => {
  globalThis.fetch = originalFetch
})

describe('device session API', () => {
  test('lists and revokes sessions through account-scoped endpoints', async () => {
    const fetchMock = mock(async (path: string) =>
      String(path).endsWith('/sessions')
        ? Response.json([
            {
              id: 'abc123',
              device_name: 'Firefox on Windows',
              ip_hint: null,
              created_at: '2026-08-27T10:00:00Z',
              last_used_at: '2026-08-27T11:00:00Z',
              expires_at: '2026-09-26T10:00:00Z',
              current: true,
            },
          ])
        : new Response(null, { status: 204 }),
    )
    globalThis.fetch = fetchMock as typeof fetch

    const sessions = await listDeviceSessions('session-token')
    await revokeDeviceSession('session-token', 'device/id')
    await revokeOtherDeviceSessions('session-token')

    expect(sessions[0]?.current).toBe(true)
    expect(String(fetchMock.mock.calls[0]![0])).toBe('/api/users/me/sessions')
    expect(String(fetchMock.mock.calls[1]![0])).toBe('/api/users/me/sessions/device%2Fid')
    expect(fetchMock.mock.calls[1]![1]?.method).toBe('DELETE')
    expect(String(fetchMock.mock.calls[2]![0])).toBe('/api/users/me/sessions/others')
  })

  test('reports an expired login instead of returning an empty list', async () => {
    globalThis.fetch = mock(async () => new Response(null, { status: 401 })) as typeof fetch
    expect(listDeviceSessions('expired-token')).rejects.toThrow('登录已过期')
  })
})
