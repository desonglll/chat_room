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
import { t } from '../../i18n/index'

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
      <h1 className="tg-login__title">{t('w.auth.b6c237')}</h1>
      <p className="tg-login__subtitle">{recovering ? t('w.auth.3a6a21', mailedTo) : t('w.auth.061b54')}</p>
      {recovering ? (
        <>
          <TextField
            label={t('w.auth.6a7140')}
            value={code}
            onChange={(event) => setCode(event.target.value)}
            name="recovery-code"
            inputMode="numeric"
            autoComplete="one-time-code"
            maxLength={6}
            hint={t('w.auth.828f74')}
            required
            fullWidth
            autoFocus
          />
        </>
      ) : (
        <TextField
          label={t('w.auth.4e69e7')}
          type="password"
          value={password}
          onChange={(event) => setPassword(event.target.value)}
          name="two-factor-password"
          autoComplete="off"
          hint={challenge.hint ? t('w.auth.e65485', challenge.hint) : undefined}
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
        {recovering ? t('w.auth.008f66') : t('w.auth.ea0ef2')}
      </Button>
      <div className="tg-login__switch tg-two-factor-login__links">
        {!recovering && challenge.has_recovery_email ? (
          <Button variant="text" onClick={startRecovery} disabled={busy} fullWidth>
            {t('w.auth.01abfd')}
          </Button>
        ) : null}
        {!recovering && !challenge.has_recovery_email ? (
          <p className="tg-two-factor-login__note">{t('w.auth.9baf44')}</p>
        ) : null}
        <Button variant="text" onClick={() => onRestart()} disabled={busy} fullWidth>
          {t('w.auth.11d024')}
        </Button>
      </div>
    </form>
  )
}
