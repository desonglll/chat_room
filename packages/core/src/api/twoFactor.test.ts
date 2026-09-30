// TG-506: the two-factor client against an injected fake fetch.
import { describe, expect, test } from 'bun:test'
import { ApiError, createApiClient } from './http'
import {
  beginLogin,
  changeTwoFactor,
  completeTwoFactorLogin,
  confirmTwoFactorRecovery,
  disableTwoFactor,
  enableTwoFactor,
} from './twoFactor'

interface Call {
  url: string
  init: RequestInit | undefined
}

function fake(respond: (url: string) => Response) {
  const calls: Call[] = []
  const client = createApiClient({
    fetchImpl: async (url, init) => {
      calls.push({ url, init })
      return respond(url)
    },
  })
  return { calls, client }
}

const session = { token: 't', user: { id: 'u' }, expires_at: '2099-01-01T00:00:00Z' }
const challenge = {
  error: 'two_factor_required',
  pending_token: 'p',
  hint: 'h',
  has_recovery_email: true,
  expires_at: '2099-01-01T00:00:00Z',
}

describe('two-factor api', () => {
  test('beginLogin passes an ordinary session through unchanged', async () => {
    const { calls, client } = fake(() => Response.json(session))
    const outcome = await beginLogin(client, 'alice', 'pw-12345678')
    expect(outcome).toEqual({ kind: 'session', session: session as never })
    expect(calls[0]!.url).toBe('/api/users/login')
  })

  test('beginLogin turns a 428 into a challenge instead of an error', async () => {
    const { client } = fake(() => Response.json(challenge, { status: 428 }))
    const outcome = await beginLogin(client, 'alice', 'pw-12345678')
    expect(outcome.kind).toBe('two_factor')
    if (outcome.kind === 'two_factor') expect(outcome.challenge.pending_token).toBe('p')
  })

  test('beginLogin still throws for a wrong account password', async () => {
    const { client } = fake(() => new Response(null, { status: 401 }))
    await expect(beginLogin(client, 'alice', 'bad')).rejects.toBeInstanceOf(ApiError)
  })

  test('stage two and recovery post the pending token, never a bearer header', async () => {
    const { calls, client } = fake(() => Response.json(session))
    await completeTwoFactorLogin(client, 'p', 'second')
    await confirmTwoFactorRecovery(client, 'p', '123456')
    expect(calls.map((c) => c.url)).toEqual([
      '/api/users/login/two-factor',
      '/api/users/login/two-factor/recovery/confirm',
    ])
    expect(JSON.parse(String(calls[0]!.init?.body))).toEqual({ pending_token: 'p', password: 'second' })
    expect(JSON.parse(String(calls[1]!.init?.body))).toEqual({ pending_token: 'p', code: '123456' })
    for (const call of calls) expect((call.init?.headers as Record<string, string>).Authorization).toBeUndefined()
  })

  test('settings calls use the bearer session and the frozen verbs', async () => {
    const { calls, client } = fake(() => Response.json({ enabled: true }))
    await enableTwoFactor(client, 's', { account_password: 'a', password: 'b', hint: '' })
    await changeTwoFactor(client, 's', { current_password: 'b', hint: 'x' })
    await disableTwoFactor(client, 's', 'b')
    expect(calls.map((c) => `${c.init?.method} ${c.url}`)).toEqual([
      'POST /api/users/me/two-factor',
      'PUT /api/users/me/two-factor',
      'DELETE /api/users/me/two-factor',
    ])
    expect((calls[2]!.init?.headers as Record<string, string>).Authorization).toBe('Bearer s')
  })
})
