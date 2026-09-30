/**
 * The second login stage (TG-506): the account password was right, the account has a
 * cloud password. Shows the hint, takes the 2FA password, and offers the email reset
 * when a verified recovery address exists. Rendered inside `LoginPage`'s card.
 */
import type { FormEvent } from 'react'
import { useState } from 'react'
import type { TwoFactorChallenge } from '@tg/core'
import { authStore, completeTwoFactorLogin, confirmTwoFactorRecovery, requestTwoFactorRecovery } from '@tg/core'
import { Button, TextField } from '@tg/ui'
import { apiClient } from '../../app/client'
import { browserStorage } from '../../app/platform'
import { isChallengeGone, secondStageErrorCopy, type SecondStageAction } from './twoFactorCopy'
import { finishLogin } from './twoStepLogin'

export interface TwoFactorLoginStepProps {
  challenge: TwoFactorChallenge
  /** The pending token is gone (expired, spent) or the user backs out: return to stage one. */
  onRestart: (notice?: string) => void
}

const deps = { client: apiClient, storage: browserStorage, store: authStore }

export function TwoFactorLoginStep({ challenge, onRestart }: TwoFactorLoginStepProps) {
  const [password, setPassword] = useState('')
  const [code, setCode] = useState('')
  const [mailedTo, setMailedTo] = useState('')
  const [error, setError] = useState('')
  const [busy, setBusy] = useState(false)
  const recovering = mailedTo !== ''

  async function run(action: SecondStageAction, work: () => Promise<void>) {
    if (busy) return
    setError('')
    setBusy(true)
    try {
      await work()
    } catch (failure) {
      const copy = secondStageErrorCopy(failure, action)
      if (isChallengeGone(failure)) onRestart(copy)
      else setError(copy)
    } finally {
      setBusy(false)
    }
  }

  function submit(event: FormEvent) {
    event.preventDefault()
    if (recovering) {
      void run('recovery-code', async () => {
        await finishLogin(deps, () => confirmTwoFactorRecovery(apiClient, challenge.pending_token, code.trim()))
      })
    } else {
      void run('password', async () => {
        await finishLogin(deps, () => completeTwoFactorLogin(apiClient, challenge.pending_token, password))
      })
    }
  }

  function startRecovery() {
    void run('recovery-request', async () => {
      const mailed = await requestTwoFactorRecovery(apiClient, challenge.pending_token)
      setMailedTo(mailed.email_pattern)
    })
  }

  return (
    <form className="tg-login__card" onSubmit={submit}>
      <h1 className="tg-login__title">两步验证</h1>
      <p className="tg-login__subtitle">
        {recovering ? `验证码已发送到 ${mailedTo}` : '这个账号开启了两步验证，请输入你设置的额外密码'}
      </p>
      {recovering ? (
        <>
          <TextField
            label="邮件验证码"
            value={code}
            onChange={(event) => setCode(event.target.value)}
            name="recovery-code"
            inputMode="numeric"
            autoComplete="one-time-code"
            maxLength={6}
            hint="验证通过后两步验证会被关闭，其他设备将全部退出登录"
            required
            fullWidth
            autoFocus
          />
        </>
      ) : (
        <TextField
          label="两步验证密码"
          type="password"
          value={password}
          onChange={(event) => setPassword(event.target.value)}
          name="two-factor-password"
          autoComplete="off"
          hint={challenge.hint ? `提示：${challenge.hint}` : undefined}
          required
          fullWidth
          autoFocus
        />
      )}
      {error ? (
        <p className="tg-login__error" role="alert">
          {error}
        </p>
      ) : null}
      <Button type="submit" loading={busy} fullWidth size="lg">
        {recovering ? '验证并登录' : '下一步'}
      </Button>
      <div className="tg-login__switch tg-two-factor-login__links">
        {!recovering && challenge.has_recovery_email ? (
          <Button variant="text" onClick={startRecovery} disabled={busy} fullWidth>
            忘记密码？通过恢复邮箱重置
          </Button>
        ) : null}
        {!recovering && !challenge.has_recovery_email ? (
          <p className="tg-two-factor-login__note">未设置恢复邮箱：忘记两步验证密码将无法登录</p>
        ) : null}
        <Button variant="text" onClick={() => onRestart()} disabled={busy} fullWidth>
          返回
        </Button>
      </div>
    </form>
  )
}
