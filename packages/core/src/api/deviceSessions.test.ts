// TG-110: the device-session client against an injected fake fetch.
import { expect, test } from 'bun:test'
import { createApiClient } from './http'
import { listDeviceSessions, revokeDeviceSession, revokeOtherDeviceSessions } from './deviceSessions'

function fake(respond: (url: string, init: RequestInit | undefined) => Response) {
  const calls: Array<{ url: string; method: string | undefined; auth: string | undefined }> = []
  const client = createApiClient({
    fetchImpl: async (url, init) => {
      const headers = (init?.headers ?? {}) as Record<string, string>
      calls.push({ url: String(url), method: init?.method, auth: headers.Authorization })
      return respond(String(url), init)
    },
  })
  return { calls, client }
}

const row = {
  id: 'a'.repeat(32),
  device_name: 'Firefox on Linux',
  ip_hint: '203.0.113.x',
  created_at: '2026-10-01T08:00:00Z',
  last_used_at: '2026-10-01T09:00:00Z',
  expires_at: '2026-10-31T08:00:00Z',
  current: true,
}

test('lists the account sessions with the bearer token', async () => {
  const { calls, client } = fake(() => Response.json([row]))
  expect(await listDeviceSessions(client, 'tok')).toEqual([row])
  expect(calls[0]).toEqual({ url: '/api/users/me/sessions', method: 'GET', auth: 'Bearer tok' })
})

test('revokes one session by its management id; an already-gone session is not an error', async () => {
  const { calls, client } = fake(() => new Response(null, { status: 404 }))
  await revokeDeviceSession(client, 'tok', 'b'.repeat(32))
  expect(calls[0]).toEqual({ url: `/api/users/me/sessions/${'b'.repeat(32)}`, method: 'DELETE', auth: 'Bearer tok' })
})

test('revoking the current session (409) surfaces as an error', async () => {
  const { client } = fake(() => new Response(null, { status: 409 }))
  await expect(revokeDeviceSession(client, 'tok', 'c'.repeat(32))).rejects.toThrow()
})

test('terminates every other session in one call', async () => {
  const { calls, client } = fake(() => new Response(null, { status: 204 }))
  await revokeOtherDeviceSessions(client, 'tok')
  expect(calls[0]).toEqual({ url: '/api/users/me/sessions/others', method: 'DELETE', auth: 'Bearer tok' })
})
