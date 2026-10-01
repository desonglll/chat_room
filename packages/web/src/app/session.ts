/**
 * Session lifecycle: persist an `AuthSession` in the injected storage, restore it at
 * boot, revalidate it against the server, and run login/register/logout. Framework-free
 * on purpose — every dependency is a parameter, so `bun test` drives the whole lifecycle
 * with a fake storage and a fake fetch (`session.test.ts`).
 */
import { clearOfflineData } from './pwa'
import type { ApiClient, AuthSession, AuthStore, CoreStorage } from '@tg/core'
import { ApiError, getCurrentUser, loginUser, logoutUser, registerUser } from '@tg/core'

export const SESSION_STORAGE_KEY = 'tg.session.v1'

export interface SessionDeps {
  client: ApiClient
  storage: CoreStorage
  store: AuthStore
}

/** The stored session, or null when absent, unreadable, or already expired. */
export function readStoredSession(storage: CoreStorage, nowMs: number): AuthSession | null {
  const raw = storage.getItem(SESSION_STORAGE_KEY)
  if (!raw) return null
  try {
    const session = JSON.parse(raw) as AuthSession
    if (!session?.token || !session.user?.id) return null
    if (Date.parse(session.expires_at) <= nowMs) return null
    return session
  } catch {
    return null
  }
}

/** Boot path: restore an unexpired stored session into the auth store. */
export function hydrateSession({ storage, store }: Omit<SessionDeps, 'client'>, nowMs: number): AuthSession | null {
  const session = readStoredSession(storage, nowMs)
  if (session) store.getState().setSession(session)
  return session
}

/**
 * Confirm a restored token is still accepted. A 401 clears the session (it was revoked
 * server-side); network failures change nothing — the next API call surfaces them.
 */
export async function revalidateSession({ client, storage, store }: SessionDeps): Promise<void> {
  const session = store.getState().session
  if (!session) return
  try {
    store.getState().updateUser(await getCurrentUser(client, session.token))
  } catch (error) {
    if (error instanceof ApiError && error.status === 401) {
      storage.removeItem(SESSION_STORAGE_KEY)
      store.getState().clearSession()
    }
  }
}

export type AuthMode = 'login' | 'register'

export interface SignInInput {
  mode: AuthMode
  username: string
  password: string
  inviteToken?: string
}

/** Login or register, persist on success. Errors re-throw after resetting the status. */
export async function signIn(deps: SessionDeps, input: SignInInput): Promise<AuthSession> {
  const { client, storage, store } = deps
  store.getState().setAuthenticating()
  try {
    const session =
      input.mode === 'register'
        ? await registerUser(client, input.username, input.password, input.inviteToken ?? '')
        : await loginUser(client, input.username, input.password)
    storage.setItem(SESSION_STORAGE_KEY, JSON.stringify(session))
    store.getState().setSession(session)
    return session
  } catch (error) {
    store.getState().clearSession()
    throw error
  }
}

/** Best-effort server logout; the local session is gone either way. */
export async function signOut({ client, storage, store }: SessionDeps): Promise<void> {
  const token = store.getState().session?.token
  storage.removeItem(SESSION_STORAGE_KEY)
  store.getState().clearSession()
  // TG-601: nothing of this account stays readable offline.
  clearOfflineData()
  if (!token) return
  try {
    await logoutUser(client, token)
  } catch {
    // The server session outliving a failed logout call expires on its own.
  }
}
