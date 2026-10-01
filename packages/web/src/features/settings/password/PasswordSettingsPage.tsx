/**
 * TG-704 «密码与账号»: change the login password (every other device is signed out by the
 * server), and delete the account after re-entering the password.
 */
import { useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { ApiError, authStore, changePassword, deleteAccount, selectToken } from '@tg/core'
import { Button, TextField } from '@tg/ui'
import { apiClient } from '../../../app/client'
import { browserStorage } from '../../../app/platform'
import { signOut } from '../../../app/session'
import { t } from '../../../i18n/index'
import { newPasswordProblem } from './passwordRules'

const token = () => selectToken(authStore.getState()) || ''

function failure(error: unknown): string {
  if (error instanceof ApiError && error.status === 401) return t('w.password.wrongCurrent')
  if (error instanceof ApiError && error.status === 400) return t('w.password.rejected')
  return t('w.password.failed')
}

export function PasswordSettingsPage() {
  const [current, setCurrent] = useState('')
  const [next, setNext] = useState('')
  const [repeat, setRepeat] = useState('')
  const [deletePassword, setDeletePassword] = useState('')
  const [confirmDelete, setConfirmDelete] = useState(false)
  const [busy, setBusy] = useState(false)
  const [message, setMessage] = useState('')
  const [error, setError] = useState('')
  const navigate = useNavigate()
  const problem = newPasswordProblem(current, next, repeat)

  const submit = () => {
    setBusy(true)
    setError('')
    changePassword(apiClient, token(), current, next)
      .then(
        () => {
          setMessage(t('w.password.changed'))
          setCurrent('')
          setNext('')
          setRepeat('')
        },
        (reason: unknown) => setError(failure(reason)),
      )
      .finally(() => setBusy(false))
  }
  const remove = () => {
    setBusy(true)
    setError('')
    deleteAccount(apiClient, token(), deletePassword)
      .then(
        async () => {
          await signOut({ client: apiClient, storage: browserStorage, store: authStore })
          void navigate('/login')
        },
        (reason: unknown) => setError(failure(reason)),
      )
      .finally(() => setBusy(false))
  }

  return (
    <div className="tg-password-settings">
      <form
        className="tg-password-settings__card"
        aria-label={t('w.password.change')}
        onSubmit={(event) => {
          event.preventDefault()
          if (!problem) submit()
        }}
      >
        <h3>{t('w.password.change')}</h3>
        <TextField
          type="password"
          autoComplete="current-password"
          label={t('w.password.current')}
          value={current}
          onChange={(e) => setCurrent(e.target.value)}
        />
        <TextField
          type="password"
          autoComplete="new-password"
          label={t('w.password.new')}
          hint={t('w.password.rule')}
          value={next}
          onChange={(e) => setNext(e.target.value)}
        />
        <TextField
          type="password"
          autoComplete="new-password"
          label={t('w.password.repeat')}
          value={repeat}
          onChange={(e) => setRepeat(e.target.value)}
          error={repeat && problem === 'w.password.mismatch' ? t(problem) : undefined}
        />
        <p className="tg-password-settings__note">{t('w.password.othersSignedOut')}</p>
        <Button type="submit" disabled={Boolean(problem)} loading={busy}>
          {t('w.password.save')}
        </Button>
      </form>

      <section className="tg-password-settings__card" aria-label={t('w.password.deleteAccount')}>
        <h3>{t('w.password.deleteAccount')}</h3>
        <p className="tg-password-settings__note">{t('w.password.deleteWarning')}</p>
        {confirmDelete ? (
          <>
            <TextField
              type="password"
              autoComplete="current-password"
              label={t('w.password.current')}
              value={deletePassword}
              onChange={(e) => setDeletePassword(e.target.value)}
            />
            <Button variant="danger" disabled={!deletePassword} loading={busy} onClick={remove}>
              {t('w.password.deleteForever')}
            </Button>
          </>
        ) : (
          <Button variant="danger" onClick={() => setConfirmDelete(true)}>
            {t('w.password.deleteAccount')}
          </Button>
        )}
      </section>
      {message ? <p role="status">{message}</p> : null}
      {error ? <p role="alert">{error}</p> : null}
    </div>
  )
}
