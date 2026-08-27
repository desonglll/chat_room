import { afterEach, describe, expect, it, mock } from 'bun:test'
import { registerUser } from './accountAuthApi'

afterEach(() => mock.restore())

describe('account registration API', () => {
  it('sends the one-time invitation only in the request body', async () => {
    const fetchMock = mock(() => Promise.resolve(new Response(JSON.stringify({ token: 'session' }), { status: 201 })))
    globalThis.fetch = fetchMock as unknown as typeof fetch

    await registerUser('alice', 'test-password', 'egi_secret')

    const [path, options] = fetchMock.mock.calls[0] as unknown as [string, RequestInit]
    expect(path).toBe('/api/users/register')
    expect(JSON.parse(String(options.body))).toEqual({
      username: 'alice',
      password: 'test-password',
      invite_token: 'egi_secret',
    })
  })
})
