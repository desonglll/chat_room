/** Session lifecycle against a fake storage and a fake fetch (no DOM, no network). */
import { describe, expect, test } from 'bun:test'
import type { AuthSession, CoreStorage, FetchLike } from '@tg/core'
import { ApiError, createApiClient, createAuthStore } from '@tg/core'
import {
  SESSION_STORAGE_KEY,
  hydrateSession,
  readStoredSession,
  revalidateSession,
  signIn,
  signOut,
} from '../src/app/session'
import { authErrorCopy } from '../src/features/auth/authCopy'

class FakeStorage implements CoreStorage {
  map = new Map<string, string>()
  getItem = (key: string) => this.map.get(key) ?? null
  setItem = (key: string, value: string) => void this.map.set(key, value)
  removeItem = (key: string) => void this.map.delete(key)
}

const user = {
  id: 'u1',
  username: 'mika',
  avatar_emoji: '',
  display_name: '',
  signature: '',
  homepage: '',
  created_at: '2026-09-30T00:00:00Z',
}

const session: AuthSession = { token: 'tok-1', user, expires_at: '2126-01-01T00:00:00Z' }

function clientOf(handler: (url: string, init?: RequestInit) => Response | Promise<Response>) {
  const calls: Array<{ url: string; init?: RequestInit }> = []
  const fetchImpl: FetchLike = async (url, init) => {
    calls.push({ url, ...(init ? { init } : {}) })
    return handler(url, init)
  }
  return { client: createApiClient({ fetchImpl }), calls }
}

describe('stored session', () => {
  test('valid JSON with a future expiry restores; garbage and expired do not', () => {
    const storage = new FakeStorage()
    storage.setItem(SESSION_STORAGE_KEY, JSON.stringify(session))
    expect(readStoredSession(storage, Date.now())?.token).toBe('tok-1')

    storage.setItem(SESSION_STORAGE_KEY, '{not json')
    expect(readStoredSession(storage, Date.now())).toBeNull()

    storage.setItem(SESSION_STORAGE_KEY, JSON.stringify({ ...session, expires_at: '2020-01-01T00:00:00Z' }))
    expect(readStoredSession(storage, Date.now())).toBeNull()
  })

  test('hydrateSession pushes the restored session into the store', () => {
    const storage = new FakeStorage()
    storage.setItem(SESSION_STORAGE_KEY, JSON.stringify(session))
    const store = createAuthStore()
    hydrateSession({ storage, store }, Date.now())
    expect(store.getState().status).toBe('authenticated')
    expect(store.getState().session?.user.username).toBe('mika')
  })
})

describe('signIn / signOut', () => {
  test('login persists the session and flips the store', async () => {
    const storage = new FakeStorage()
    const store = createAuthStore()
    const { client, calls } = clientOf(() => Response.json(session))
    const result = await signIn({ client, storage, store }, { mode: 'login', username: 'mika', password: 'pw123456' })
    expect(result.token).toBe('tok-1')
    expect(calls[0]?.url).toBe('/api/users/login')
    expect(store.getState().status).toBe('authenticated')
    expect(JSON.parse(storage.getItem(SESSION_STORAGE_KEY) ?? '{}').token).toBe('tok-1')
  })

  test('register hits the register endpoint with the invite token', async () => {
    const storage = new FakeStorage()
    const store = createAuthStore()
    const { client, calls } = clientOf(() => Response.json(session))
    await signIn(
      { client, storage, store },
      { mode: 'register', username: 'mika', password: 'pw123456', inviteToken: 'inv-1' },
    )
    expect(calls[0]?.url).toBe('/api/users/register')
    expect(JSON.parse(String(calls[0]?.init?.body))).toMatchObject({ username: 'mika', invite_token: 'inv-1' })
  })

  test('a rejected login clears the store and surfaces an ApiError', async () => {
    const storage = new FakeStorage()
    const store = createAuthStore()
    const { client } = clientOf(() => Response.json({ error: 'nope' }, { status: 401 }))
    await expect(
      signIn({ client, storage, store }, { mode: 'login', username: 'mika', password: 'wrong' }),
    ).rejects.toBeInstanceOf(ApiError)
    expect(store.getState().status).toBe('anonymous')
    expect(storage.getItem(SESSION_STORAGE_KEY)).toBeNull()
  })

  test('signOut clears locally even when the server call fails', async () => {
    const storage = new FakeStorage()
    const store = createAuthStore()
    store.getState().setSession(session)
    storage.setItem(SESSION_STORAGE_KEY, JSON.stringify(session))
    const { client } = clientOf(() => Response.json({}, { status: 500 }))
    await signOut({ client, storage, store })
    expect(store.getState().session).toBeNull()
    expect(storage.getItem(SESSION_STORAGE_KEY)).toBeNull()
  })
})

describe('revalidateSession', () => {
  test('a 401 clears the restored session; a network failure keeps it', async () => {
    const storage = new FakeStorage()
    const store = createAuthStore()
    store.getState().setSession(session)
    storage.setItem(SESSION_STORAGE_KEY, JSON.stringify(session))

    const revoked = clientOf(() => Response.json({}, { status: 401 }))
    await revalidateSession({ client: revoked.client, storage, store })
    expect(store.getState().session).toBeNull()
    expect(storage.getItem(SESSION_STORAGE_KEY)).toBeNull()

    store.getState().setSession(session)
    const flaky = createApiClient({
      fetchImpl: async () => {
        throw new TypeError('network down')
      },
    })
    await revalidateSession({ client: flaky, storage, store })
    expect(store.getState().session?.token).toBe('tok-1')
  })
})

describe('authErrorCopy', () => {
  test('maps the auth statuses to Chinese copy without matching server strings', () => {
    expect(authErrorCopy(new ApiError(401, '/api/users/login', 'x'), 'login')).toBe('用户名或密码不正确')
    expect(authErrorCopy(new ApiError(409, '/api/users/register', 'x'), 'register')).toBe('这个用户名已被使用')
    expect(authErrorCopy(new ApiError(403, '/api/users/register', 'x'), 'register')).toContain('邀请')
    expect(authErrorCopy(new ApiError(429, '/api/users/login', 'x'), 'login')).toContain('稍后')
    expect(authErrorCopy(new ApiError(503, '/api/users/login', 'x'), 'login')).toContain('503')
    expect(authErrorCopy(new TypeError('offline'), 'login')).toContain('网络')
  })
})
