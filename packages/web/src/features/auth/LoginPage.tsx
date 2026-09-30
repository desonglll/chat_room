/**
 * Login / register — the door into the shell. One centred card, two modes, no
 * flourish: the whole page is `@tg/ui` primitives on the token layer.
 *
 * The registration mode of the deployment (`GET /api/config`) decides whether the
 * register tab shows an invite-code field (`invite_only`) or is disabled entirely.
 *
 * TG-506: login is two-stage for accounts with a cloud password — a `428` from stage one
 * swaps the card for `TwoFactorLoginStep`; accounts without 2FA sign in as before.
 */
import type { FormEvent } from 'react'
import { useEffect, useState } from 'react'
import { getPublicConfig, type RegistrationMode, type TwoFactorChallenge } from '@tg/core'
import { authStore } from '@tg/core'
import { Button, TextField } from '@tg/ui'
import { apiClient } from '../../app/client'
import { browserStorage } from '../../app/platform'
import type { AuthMode } from '../../app/session'
import { signIn } from '../../app/session'
import { authErrorCopy } from './authCopy'
import { TwoFactorLoginStep } from './TwoFactorLoginStep'
import { startLogin } from './twoStepLogin'

export function LoginPage() {
  const [mode, setMode] = useState<AuthMode>('login')
  const [username, setUsername] = useState('')
  const [password, setPassword] = useState('')
  const [inviteToken, setInviteToken] = useState('')
  const [error, setError] = useState('')
  const [busy, setBusy] = useState(false)
  const [registration, setRegistration] = useState<RegistrationMode>('open')
  const [challenge, setChallenge] = useState<TwoFactorChallenge | null>(null)

  useEffect(() => {
    let cancelled = false
    getPublicConfig(apiClient)
      .then((config) => {
        if (!cancelled) setRegistration(config.registration_mode)
      })
      .catch(() => {
        // Unknown deployment config: keep the open-registration default.
      })
    return () => {
      cancelled = true
    }
  }, [])

  const registerDisabled = registration === 'disabled'
  const showInvite = mode === 'register' && registration === 'invite_only'

  async function submit(event: FormEvent) {
    event.preventDefault()
    if (busy) return
    setError('')
    setBusy(true)
    const deps = { client: apiClient, storage: browserStorage, store: authStore }
    try {
      if (mode === 'login') {
        const outcome = await startLogin(deps, username.trim(), password)
        if (outcome.kind === 'two_factor') {
          setPassword('')
          setChallenge(outcome.challenge)
        }
      } else {
        await signIn(deps, { mode, username: username.trim(), password, inviteToken: inviteToken.trim() })
      }
      // Success flips authStore; the router's AnonymousOnly guard leaves this page.
    } catch (failure) {
      setError(authErrorCopy(failure, mode))
    } finally {
      setBusy(false)
    }
  }

  function switchMode(next: AuthMode) {
    setMode(next)
    setError('')
  }

  function restart(notice?: string) {
    setChallenge(null)
    setError(notice ?? '')
  }

  if (challenge) {
    return (
      <main className="tg-login">
        <TwoFactorLoginStep challenge={challenge} onRestart={restart} />
      </main>
    )
  }

  return (
    <main className="tg-login">
      <form className="tg-login__card" onSubmit={submit}>
        <h1 className="tg-login__title">Echo Gate</h1>
        <p className="tg-login__subtitle">{mode === 'login' ? '登录以继续你的会话' : '创建账号，开始第一段会话'}</p>
        <TextField
          label="用户名"
          value={username}
          onChange={(event) => setUsername(event.target.value)}
          autoComplete="username"
          name="username"
          maxLength={48}
          required
          fullWidth
          autoFocus
        />
        <TextField
          label="密码"
          type="password"
          value={password}
          onChange={(event) => setPassword(event.target.value)}
          autoComplete={mode === 'login' ? 'current-password' : 'new-password'}
          name="password"
          hint={mode === 'register' ? '至少 8 位' : undefined}
          required
          fullWidth
        />
        {showInvite ? (
          <TextField
            label="邀请码"
            value={inviteToken}
            onChange={(event) => setInviteToken(event.target.value)}
            name="invite"
            required
            fullWidth
          />
        ) : null}
        {error ? (
          <p className="tg-login__error" role="alert">
            {error}
          </p>
        ) : null}
        <Button type="submit" loading={busy} fullWidth size="lg">
          {mode === 'login' ? '登录' : '注册'}
        </Button>
        <div className="tg-login__switch">
          {mode === 'login' ? (
            <Button variant="text" onClick={() => switchMode('register')} disabled={registerDisabled} fullWidth>
              {registerDisabled ? '当前部署已关闭注册' : '注册新账号'}
            </Button>
          ) : (
            <Button variant="text" onClick={() => switchMode('login')} fullWidth>
              已有账号？登录
            </Button>
          )}
        </div>
      </form>
    </main>
  )
}
