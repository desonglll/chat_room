/** TG-506: two-stage login controller, settings rules, copy, and initial markup. */
import { describe, expect, test } from 'bun:test'
import { renderToStaticMarkup } from 'react-dom/server'
import type { AuthSession, CoreStorage, FetchLike } from '@tg/core'
import { ApiError, completeTwoFactorLogin, createApiClient, createAuthStore } from '@tg/core'
import { SESSION_STORAGE_KEY } from '../src/app/session'
import { TwoFactorLoginStep } from '../src/features/auth/TwoFactorLoginStep'
import { isChallengeGone, secondStageErrorCopy } from '../src/features/auth/twoFactorCopy'
import { finishLogin, startLogin } from '../src/features/auth/twoStepLogin'
import { TwoFactorSettings } from '../src/features/settings/security'
import {
  checkCode,
  checkEmail,
  checkHint,
  checkNewPassword,
  securityErrorCopy,
} from '../src/features/settings/security/securityRules'

class FakeStorage implements CoreStorage {
  map = new Map<string, string>()
  getItem = (key: string) => this.map.get(key) ?? null
  setItem = (key: string, value: string) => void this.map.set(key, value)
  removeItem = (key: string) => void this.map.delete(key)
}

const session = {
  token: 'tok-2fa',
  user: { id: 'u1', username: 'mika' },
  expires_at: '2126-01-01T00:00:00Z',
} as AuthSession
const challenge = {
  error: 'two_factor_required' as const,
  pending_token: 'pending-1',
  hint: '最喜欢的电影',
  has_recovery_email: true,
  expires_at: '2126-01-01T00:00:00Z',
}

function depsOf(handler: (url: string) => Response) {
  const fetchImpl: FetchLike = async (url) => handler(url)
  return { client: createApiClient({ fetchImpl }), storage: new FakeStorage(), store: createAuthStore() }
}

describe('two-stage login', () => {
  test('an account without 2FA is persisted exactly like signIn', async () => {
    const deps = depsOf(() => Response.json(session))
    const outcome = await startLogin(deps, 'mika', 'password-1')
    expect(outcome.kind).toBe('session')
    expect(deps.store.getState().status).toBe('authenticated')
    expect(JSON.parse(deps.storage.getItem(SESSION_STORAGE_KEY)!).token).toBe('tok-2fa')
  })

  test('a 428 persists nothing and leaves the store anonymous for stage two', async () => {
    const deps = depsOf(() => Response.json(challenge, { status: 428 }))
    const outcome = await startLogin(deps, 'mika', 'password-1')
    expect(outcome.kind).toBe('two_factor')
    expect(deps.store.getState().status).toBe('anonymous')
    expect(deps.storage.getItem(SESSION_STORAGE_KEY)).toBeNull()
  })

  test('stage two persists the session it earns', async () => {
    const deps = depsOf(() => Response.json(session))
    await finishLogin(deps, () => completeTwoFactorLogin(deps.client, 'pending-1', 'second'))
    expect(deps.store.getState().session?.token).toBe('tok-2fa')
    expect(deps.storage.getItem(SESSION_STORAGE_KEY)).not.toBeNull()
  })

  test('a wrong account password still fails and clears the store', async () => {
    const deps = depsOf(() => new Response(null, { status: 401 }))
    await expect(startLogin(deps, 'mika', 'bad-password')).rejects.toBeInstanceOf(ApiError)
    expect(deps.store.getState().status).toBe('anonymous')
  })

  test('copy distinguishes a wrong password, a wrong code and a dead challenge', () => {
    const at = (status: number) => new ApiError(status, '/x', '')
    expect(secondStageErrorCopy(at(401), 'password')).toBe('两步验证密码不正确')
    expect(secondStageErrorCopy(at(401), 'recovery-code')).toBe('验证码不正确')
    expect(secondStageErrorCopy(at(429), 'password')).toContain('稍后')
    expect(isChallengeGone(at(410))).toBe(true)
    expect(isChallengeGone(at(401))).toBe(false)
  })
})

describe('settings rules', () => {
  test('new passwords must match and the hint may not reveal them', () => {
    expect(checkNewPassword({ password: '', confirm: '', hint: '' })).not.toBe('')
    expect(checkNewPassword({ password: 'abc', confirm: 'abd', hint: '' })).toBe('两次输入的密码不一致')
    expect(checkNewPassword({ password: 'secret', confirm: 'secret', hint: 'my SECRET' })).toBe(
      '提示中不能包含密码本身',
    )
    expect(checkNewPassword({ password: 'secret', confirm: 'secret', hint: '小狗' })).toBe('')
    expect(checkHint('x'.repeat(65), 'pw')).not.toBe('')
  })

  test('emails and codes', () => {
    expect(checkEmail('a@example.com')).toBe('')
    for (const bad of ['a@b', '@b.cd', 'a b@c.de', 'a@@c.de']) expect(checkEmail(bad)).not.toBe('')
    expect(checkCode('012345')).toBe('')
    expect(checkCode('12345')).not.toBe('')
  })

  test('status copy follows the action', () => {
    const at = (status: number) => new ApiError(status, '/x', '')
    expect(securityErrorCopy(at(401), 'enable')).toBe('账号密码不正确')
    expect(securityErrorCopy(at(401), 'disable')).toBe('两步验证密码不正确')
    expect(securityErrorCopy(at(503), 'email')).toContain('邮件')
  })
})

describe('markup', () => {
  test('the second stage shows the hint and the recovery link', () => {
    const html = renderToStaticMarkup(<TwoFactorLoginStep challenge={challenge} onRestart={() => {}} />)
    expect(html).toContain('两步验证')
    expect(html).toContain('提示：最喜欢的电影')
    expect(html).toContain('type="password"')
    expect(html).toContain('忘记密码？')
  })

  test('without a recovery email the second stage says so instead of offering a reset', () => {
    const html = renderToStaticMarkup(
      <TwoFactorLoginStep challenge={{ ...challenge, has_recovery_email: false }} onRestart={() => {}} />,
    )
    expect(html).not.toContain('忘记密码？')
    expect(html).toContain('未设置恢复邮箱')
  })

  test('the settings panel renders its titled region while loading', () => {
    const html = renderToStaticMarkup(<TwoFactorSettings />)
    expect(html).toContain('aria-labelledby="tg-security-title"')
    expect(html).toContain('两步验证')
  })
})
