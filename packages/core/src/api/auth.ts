/**
 * Account/session endpoints (`/api/users/*`). Rewritten from `web/src/accountAuthApi.ts`
 * and the account half of `web/src/api.ts` on the shared `ApiClient` (TG-011).
 */
import type { AuthSession, UpdateProfilePayload, User } from '../types'
import type { ApiClient } from './http'

async function authenticate(
  client: ApiClient,
  endpoint: 'register' | 'login',
  username: string,
  password: string,
  inviteToken?: string,
): Promise<AuthSession> {
  return client.json<AuthSession>('POST', `/api/users/${endpoint}`, {
    body: { username, password, invite_token: inviteToken || undefined },
  })
}

export function registerUser(
  client: ApiClient,
  username: string,
  password: string,
  inviteToken = '',
): Promise<AuthSession> {
  return authenticate(client, 'register', username, password, inviteToken)
}

export function loginUser(client: ApiClient, username: string, password: string): Promise<AuthSession> {
  return authenticate(client, 'login', username, password)
}

export async function logoutUser(client: ApiClient, token: string): Promise<void> {
  // A 401 means the session is already gone — that is a successful logout.
  await client.request('POST', '/api/users/logout', { token, allowStatuses: [401] })
}

export function getCurrentUser(client: ApiClient, token: string): Promise<User> {
  return client.json<User>('GET', '/api/users/me', { token })
}

export function updateCurrentUser(client: ApiClient, token: string, payload: UpdateProfilePayload): Promise<User> {
  return client.json<User>('PATCH', '/api/users/me', { token, body: payload })
}
