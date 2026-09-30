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
          取消
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
      label="密码提示（可选）"
      value={value}
      onChange={(event) => onChange(event.target.value)}
      name="two-factor-hint"
      maxLength={MAX_HINT_CHARS}
      hint="登录时会显示给输入账号密码正确的人，不要写密码本身"
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
    run(accountPassword ? checkNewPassword({ password, confirm, hint }) : '请输入账号密码', async () => {
      onDone(
        await enableTwoFactor(apiClient, token, { account_password: accountPassword, password, hint: hint.trim() }),
      )
    })
  return (
    <Frame
      title="设置两步验证密码"
      note="开启后，除当前设备外，你的所有其他设备都会立即退出登录。"
      error={error}
      busy={busy}
      submitLabel="开启两步验证"
      onSubmit={submit}
      onCancel={onCancel}
    >
      <PasswordField label="账号密码" value={accountPassword} onChange={setAccountPassword} name="account-password" />
      <PasswordField label="两步验证密码" value={password} onChange={setPassword} name="two-factor-new" />
      <PasswordField label="再次输入两步验证密码" value={confirm} onChange={setConfirm} name="two-factor-confirm" />
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
    ? '请输入当前两步验证密码'
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
      title="修改两步验证"
      note="只改提示时，新密码两栏留空即可。"
      error={error}
      busy={busy}
      submitLabel="保存"
      onSubmit={submit}
      onCancel={onCancel}
    >
      <PasswordField label="当前两步验证密码" value={current} onChange={setCurrent} name="two-factor-current" />
      <PasswordField label="新密码（可选）" value={password} onChange={setPassword} name="two-factor-new" />
      <PasswordField label="再次输入新密码" value={confirm} onChange={setConfirm} name="two-factor-confirm" />
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
    sender.run(current ? checkEmail(email) : '请输入当前两步验证密码', async () => {
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
      title="恢复邮箱"
      note="忘记两步验证密码时，可以用发送到这个邮箱的验证码重置；重置会让其他设备全部退出登录。"
      error={sentTo ? confirmer.error || sender.error : sender.error}
      busy={busy}
      submitLabel={sentTo ? '验证邮箱' : '发送验证码'}
      onSubmit={sentTo ? verify : send}
      onCancel={onCancel}
    >
      {sentTo ? (
        <>
          <p className="tg-security__state">验证码已发送到 {sentTo}</p>
          <TextField
            label="邮件验证码"
            value={code}
            onChange={(event) => setCode(event.target.value)}
            name="recovery-code"
            inputMode="numeric"
            autoComplete="one-time-code"
            maxLength={6}
            fullWidth
          />
          <Button variant="text" onClick={() => setSentTo('')} disabled={busy} fullWidth>
            换一个邮箱或重新发送
          </Button>
        </>
      ) : (
        <>
          <PasswordField label="当前两步验证密码" value={current} onChange={setCurrent} name="two-factor-current" />
          <TextField
            label="邮箱地址"
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
    run(current ? '' : '请输入当前两步验证密码', async () => {
      await disableTwoFactor(apiClient, token, current)
      onDone(null)
    })
  return (
    <Frame
      title="关闭两步验证"
      note="关闭后，只凭账号密码就能在新设备登录，恢复邮箱也会一并移除。"
      error={error}
      busy={busy}
      submitLabel="关闭两步验证"
      danger
      onSubmit={submit}
      onCancel={onCancel}
    >
      <PasswordField label="当前两步验证密码" value={current} onChange={setCurrent} name="two-factor-current" />
    </Frame>
  )
}
