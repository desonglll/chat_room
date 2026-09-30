/**
 * The login half of `app/session.ts`'s `signIn`, made two-stage for TG-506. Framework-free
 * like `signIn`: every dependency is a parameter so `bun test` drives it with fakes.
 *
 * Stage one either yields a session (account without 2FA — persisted exactly as `signIn`
 * does) or a `TwoFactorChallenge`; the store goes back to `anonymous` while the second
 * stage is on screen. Stage two and the recovery reset end in `adoptSession`.
 */
import type { AuthSession, LoginOutcome } from '@tg/core'
import { beginLogin } from '@tg/core'
import type { SessionDeps } from '../../app/session'
import { SESSION_STORAGE_KEY } from '../../app/session'

export function adoptSession({ storage, store }: Omit<SessionDeps, 'client'>, session: AuthSession): AuthSession {
  storage.setItem(SESSION_STORAGE_KEY, JSON.stringify(session))
  store.getState().setSession(session)
  return session
}

export async function startLogin(deps: SessionDeps, username: string, password: string): Promise<LoginOutcome> {
  const { client, store } = deps
  store.getState().setAuthenticating()
  try {
    const outcome = await beginLogin(client, username, password)
    if (outcome.kind === 'session') adoptSession(deps, outcome.session)
    else store.getState().clearSession()
    return outcome
  } catch (error) {
    store.getState().clearSession()
    throw error
  }
}

/** Run a stage-two call (2FA password or recovery code) and persist what it returns. */
export async function finishLogin(deps: SessionDeps, call: () => Promise<AuthSession>): Promise<AuthSession> {
  return adoptSession(deps, await call())
}
