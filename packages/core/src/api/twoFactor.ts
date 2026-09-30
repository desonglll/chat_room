/**
 * Two-step verification ("cloud password", TG-506). Wire contract frozen in
 * `docs/devlog/TG-506.md`.
 *
 * Login is two-stage for an account with 2FA: `POST /api/users/login` answers `428` with a
 * pending token (never a session); `completeTwoFactorLogin` exchanges it plus the 2FA
 * password for the ordinary `AuthSession`. Accounts without 2FA get the unchanged `200`.
 */
import type { AuthSession } from '../types'
import type { ApiClient } from './http'

/** `428` body of `POST /api/users/login` when the account has 2FA on. */
export interface TwoFactorChallenge {
  error: 'two_factor_required'
  pending_token: string
  hint: string
  has_recovery_email: boolean
  expires_at: string
}

export type LoginOutcome =
  | { kind: 'session'; session: AuthSession }
  | { kind: 'two_factor'; challenge: TwoFactorChallenge }

export interface TwoFactorStatus {
  enabled: boolean
  hint: string
  recovery_email: string | null
  pending_recovery_email: string | null
}

/** `202` body whenever the server mailed a code. */
export interface MailedCode {
  email_pattern: string
  expires_at: string
}

export interface EnableTwoFactorInput {
  account_password: string
  password: string
  hint: string
}

export interface ChangeTwoFactorInput {
  current_password: string
  new_password?: string
  hint?: string
}

const ME = '/api/users/me/two-factor'
const LOGIN = '/api/users/login/two-factor'

/** Stage one. Same request as `loginUser`, but a `428` becomes a challenge instead of an error. */
export async function beginLogin(client: ApiClient, username: string, password: string): Promise<LoginOutcome> {
  const response = await client.request('POST', '/api/users/login', {
    body: { username, password },
    allowStatuses: [428],
  })
  if (response.status === 428) {
    return { kind: 'two_factor', challenge: (await response.json()) as TwoFactorChallenge }
  }
  return { kind: 'session', session: (await response.json()) as AuthSession }
}

/** Stage two: `401` wrong password, `410` pending token spent/expired (restart), `429` limited. */
export function completeTwoFactorLogin(
  client: ApiClient,
  pendingToken: string,
  password: string,
): Promise<AuthSession> {
  return client.json<AuthSession>('POST', LOGIN, { body: { pending_token: pendingToken, password } })
}

/** Mail a reset code to the verified recovery address (`409` none, `503` no mail transport). */
export function requestTwoFactorRecovery(client: ApiClient, pendingToken: string): Promise<MailedCode> {
  return client.json<MailedCode>('POST', `${LOGIN}/recovery`, { body: { pending_token: pendingToken } })
}

/** Reset 2FA with the mailed code; every other session of the account ends. */
export function confirmTwoFactorRecovery(client: ApiClient, pendingToken: string, code: string): Promise<AuthSession> {
  return client.json<AuthSession>('POST', `${LOGIN}/recovery/confirm`, {
    body: { pending_token: pendingToken, code },
  })
}

export function getTwoFactorStatus(client: ApiClient, token: string): Promise<TwoFactorStatus> {
  return client.json<TwoFactorStatus>('GET', ME, { token })
}

/** Turn 2FA on. The server ends every other session of the account (D-010). */
export function enableTwoFactor(
  client: ApiClient,
  token: string,
  input: EnableTwoFactorInput,
): Promise<TwoFactorStatus> {
  return client.json<TwoFactorStatus>('POST', ME, { token, body: input })
}

export function changeTwoFactor(
  client: ApiClient,
  token: string,
  input: ChangeTwoFactorInput,
): Promise<TwoFactorStatus> {
  return client.json<TwoFactorStatus>('PUT', ME, { token, body: input })
}

export async function disableTwoFactor(client: ApiClient, token: string, currentPassword: string): Promise<void> {
  await client.request('DELETE', ME, { token, body: { current_password: currentPassword } })
}

export function requestRecoveryEmail(
  client: ApiClient,
  token: string,
  currentPassword: string,
  email: string,
): Promise<MailedCode> {
  return client.json<MailedCode>('POST', `${ME}/recovery-email`, {
    token,
    body: { current_password: currentPassword, email },
  })
}

export function confirmRecoveryEmail(client: ApiClient, token: string, code: string): Promise<TwoFactorStatus> {
  return client.json<TwoFactorStatus>('POST', `${ME}/recovery-email/confirm`, { token, body: { code } })
}
