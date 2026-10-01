/**
 * The four focused forms behind `TwoFactorSettings`: enable, change, recovery email,
 * disable. Each validates locally with `securityRules.ts`, calls one `@tg/core` endpoint,
 * and reports the new status (or null to reload) through `onDone`.
 */
import type { FormEvent, ReactNode } from 'react'
import { useState } from 'react'
import type { TwoFactorStatus } from '@tg/core'
import {
  changeTwoFactor,
  confirmRecoveryEmail,
  disableTwoFactor,
  enableTwoFactor,
  requestRecoveryEmail,
} from '@tg/core'
import { Button, TextField } from '@tg/ui'
import { apiClient } from '../../../app/client'
import {
  checkCode,
  checkEmail,
  checkHint,
  checkNewPassword,
  MAX_HINT_CHARS,
  securityErrorCopy,
  type SecurityAction,
} from './securityRules'
import { t } from '../../../i18n/index'

interface FormProps {
  token: string
  onDone: (status: TwoFactorStatus | null) => void
  onCancel: () => void
}

function useSubmit(action: SecurityAction) {
  const [error, setError] = useState('')
  const [busy, setBusy] = useState(false)
  async function run(localError: string, work: () => Promise<void>) {
    if (busy) return
    setError(localError)
    if (localError) return
    setBusy(true)
    try {
      await work()
    } catch (failure) {
      setError(securityErrorCopy(failure, action))
    } finally {
      setBusy(false)
    }
  }
  return { error, busy, run, setError }
}

function Frame(props: {
  title: string
  note?: ReactNode
  error: string
  busy: boolean
  submitLabel: string
  danger?: boolean
  onSubmit: () => void
  onCancel: () => void
  children: ReactNode
}) {
  function submit(event: FormEvent) {
    event.preventDefault()
    props.onSubmit()
  }
  return (
    <form className="tg-security__form" onSubmit={submit}>
      <h3 className="tg-security__form-title">{props.title}</h3>
      {props.note ? <p className="tg-security__warning">{props.note}</p> : null}
      {props.children}
      {props.error ? (
        <p className="tg-security__error" role="alert">
          {props.error}
        </p>
      ) : null}
      <div className="tg-security__actions">
        <Button type="submit" variant={props.danger ? 'danger' : 'filled'} loading={props.busy} fullWidth>
          {props.submitLabel}
        </Button>
        <Button variant="text" onClick={props.onCancel} disabled={props.busy} fullWidth>
          {t('w.settings.4d0b46')}
        </Button>
      </div>
    </form>
  )
}

function PasswordField(props: { label: string; value: string; onChange: (value: string) => void; name: string }) {
  return (
    <TextField
      label={props.label}
      type="password"
      value={props.value}
      onChange={(event) => props.onChange(event.target.value)}
      name={props.name}
      autoComplete={props.name === 'account-password' ? 'current-password' : 'off'}
      maxLength={256}
      fullWidth
    />
  )
}

function HintField({ value, onChange }: { value: string; onChange: (value: string) => void }) {
  return (
    <TextField
      label={t('w.settings.f36cd5')}
      value={value}
      onChange={(event) => onChange(event.target.value)}
      name="two-factor-hint"
      maxLength={MAX_HINT_CHARS}
      hint={t('w.settings.1fc974')}
      fullWidth
    />
  )
}

export function EnableForm({ token, onDone, onCancel }: FormProps) {
  const [accountPassword, setAccountPassword] = useState('')
  const [password, setPassword] = useState('')
  const [confirm, setConfirm] = useState('')
  const [hint, setHint] = useState('')
  const { error, busy, run } = useSubmit('enable')
  const submit = () =>
    run(accountPassword ? checkNewPassword({ password, confirm, hint }) : t('w.settings.eb88e8'), async () => {
      onDone(
        await enableTwoFactor(apiClient, token, { account_password: accountPassword, password, hint: hint.trim() }),
      )
    })
  return (
    <Frame
      title={t('w.settings.fc2484')}
      note={t('w.settings.a1677d')}
      error={error}
      busy={busy}
      submitLabel={t('w.settings.7b4039')}
      onSubmit={submit}
      onCancel={onCancel}
    >
      <PasswordField
        label={t('w.settings.d3ef11')}
        value={accountPassword}
        onChange={setAccountPassword}
        name="account-password"
      />
      <PasswordField label={t('w.settings.4e69e7')} value={password} onChange={setPassword} name="two-factor-new" />
      <PasswordField label={t('w.settings.9c186e')} value={confirm} onChange={setConfirm} name="two-factor-confirm" />
      <HintField value={hint} onChange={setHint} />
    </Frame>
  )
}

export function ChangeForm({ token, currentHint, onDone, onCancel }: FormProps & { currentHint: string }) {
  const [current, setCurrent] = useState('')
  const [password, setPassword] = useState('')
  const [confirm, setConfirm] = useState('')
  const [hint, setHint] = useState(currentHint)
  const { error, busy, run } = useSubmit('change')
  const changingPassword = password !== '' || confirm !== ''
  const localError = !current
    ? t('w.settings.b70c9b')
    : changingPassword
      ? checkNewPassword({ password, confirm, hint })
      : checkHint(hint, current)
  const submit = () =>
    run(localError, async () => {
      onDone(
        await changeTwoFactor(apiClient, token, {
          current_password: current,
          ...(changingPassword ? { new_password: password } : {}),
          hint: hint.trim(),
        }),
      )
    })
  return (
    <Frame
      title={t('w.settings.ab02dc')}
      note={t('w.settings.9ee57c')}
      error={error}
      busy={busy}
      submitLabel={t('w.settings.fadf24')}
      onSubmit={submit}
      onCancel={onCancel}
    >
      <PasswordField label={t('w.settings.499b6d')} value={current} onChange={setCurrent} name="two-factor-current" />
      <PasswordField label={t('w.settings.95a1ae')} value={password} onChange={setPassword} name="two-factor-new" />
      <PasswordField label={t('w.settings.e03274')} value={confirm} onChange={setConfirm} name="two-factor-confirm" />
      <HintField value={hint} onChange={setHint} />
    </Frame>
  )
}

export function EmailForm({ token, pendingEmail, onDone, onCancel }: FormProps & { pendingEmail: string | null }) {
  const [current, setCurrent] = useState('')
  const [email, setEmail] = useState(pendingEmail ?? '')
  const [code, setCode] = useState('')
  const [sentTo, setSentTo] = useState(pendingEmail ? pendingEmail : '')
  const sender = useSubmit('email')
  const confirmer = useSubmit('code')
  const busy = sender.busy || confirmer.busy

  const send = () =>
    sender.run(current ? checkEmail(email) : t('w.settings.b70c9b'), async () => {
      const mailed = await requestRecoveryEmail(apiClient, token, current, email.trim())
      setSentTo(mailed.email_pattern)
      setCode('')
    })
  const verify = () =>
    confirmer.run(checkCode(code), async () => {
      onDone(await confirmRecoveryEmail(apiClient, token, code.trim()))
    })

  return (
    <Frame
      title={t('w.settings.9f8463')}
      note={t('w.settings.b2d9e8')}
      error={sentTo ? confirmer.error || sender.error : sender.error}
      busy={busy}
      submitLabel={sentTo ? t('w.settings.95e8e7') : t('w.settings.42e8ed')}
      onSubmit={sentTo ? verify : send}
      onCancel={onCancel}
    >
      {sentTo ? (
        <>
          <p className="tg-security__state">
            {t('w.settings.a6aadc')} {sentTo}
          </p>
          <TextField
            label={t('w.settings.6a7140')}
            value={code}
            onChange={(event) => setCode(event.target.value)}
            name="recovery-code"
            inputMode="numeric"
            autoComplete="one-time-code"
            maxLength={6}
            fullWidth
          />
          <Button variant="text" onClick={() => setSentTo('')} disabled={busy} fullWidth>
            {t('w.settings.b59bd5')}
          </Button>
        </>
      ) : (
        <>
          <PasswordField
            label={t('w.settings.499b6d')}
            value={current}
            onChange={setCurrent}
            name="two-factor-current"
          />
          <TextField
            label={t('w.settings.b13dda')}
            type="email"
            value={email}
            onChange={(event) => setEmail(event.target.value)}
            name="recovery-email"
            autoComplete="email"
            maxLength={254}
            fullWidth
          />
        </>
      )}
    </Frame>
  )
}

export function DisableForm({ token, onDone, onCancel }: FormProps) {
  const [current, setCurrent] = useState('')
  const { error, busy, run } = useSubmit('disable')
  const submit = () =>
    run(current ? '' : t('w.settings.b70c9b'), async () => {
      await disableTwoFactor(apiClient, token, current)
      onDone(null)
    })
  return (
    <Frame
      title={t('w.settings.f6901f')}
      note={t('w.settings.e72cb3')}
      error={error}
      busy={busy}
      submitLabel={t('w.settings.f6901f')}
      danger
      onSubmit={submit}
      onCancel={onCancel}
    >
      <PasswordField label={t('w.settings.499b6d')} value={current} onChange={setCurrent} name="two-factor-current" />
    </Frame>
  )
}
