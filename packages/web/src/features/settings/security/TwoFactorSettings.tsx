/**
 * Settings → Privacy and Security → Two-Step Verification (TG-506).
 *
 * A standalone panel: it reads the session token from `authStore` and needs no props, so
 * the integration lead can mount it anywhere in the settings column. It shows the current
 * state and switches between one overview and four focused forms (`TwoFactorForms.tsx`).
 */
import { useCallback, useEffect, useState } from 'react'
import type { TwoFactorStatus } from '@tg/core'
import { authStore, getTwoFactorStatus, selectToken } from '@tg/core'
import { Button, Spinner } from '@tg/ui'
import { useStore } from 'zustand/react'
import { apiClient } from '../../../app/client'
import { ChangeForm, DisableForm, EmailForm, EnableForm } from './TwoFactorForms'
import { securityErrorCopy, type SecurityView } from './securityRules'
import { t } from '../../../i18n/index'
// TG-1003: this screen is lazy, so its stylesheet travels with it, not in the first paint.
import './security.css'

export function TwoFactorSettings() {
  const token = useStore(authStore, selectToken)
  const [status, setStatus] = useState<TwoFactorStatus | null>(null)
  const [view, setView] = useState<SecurityView>('overview')
  const [error, setError] = useState('')
  const [notice, setNotice] = useState('')

  const reload = useCallback(async () => {
    if (!token) return
    try {
      setStatus(await getTwoFactorStatus(apiClient, token))
      setError('')
    } catch (failure) {
      setError(securityErrorCopy(failure, 'load'))
    }
  }, [token])

  useEffect(() => {
    void reload()
  }, [reload])

  function done(next: TwoFactorStatus | null, message: string) {
    if (next) setStatus(next)
    else void reload()
    setNotice(message)
    setView('overview')
  }

  function open(next: SecurityView) {
    setNotice('')
    setView(next)
  }

  const back = () => setView('overview')

  return (
    // TG-1101: the settings shell already shows «两步验证» as the page title; repeating it as a
    // heading inside the page doubled it. The section keeps it as its accessible name.
    <section className="tg-security" aria-label={t('w.settings.b6c237')}>
      <header className="tg-security__header">
        <p className="tg-security__lead">{t('w.settings.39775f')}</p>
      </header>
      {error ? (
        <p className="tg-security__error" role="alert">
          {error}
        </p>
      ) : null}
      {notice ? (
        <p className="tg-security__notice" role="status">
          {notice}
        </p>
      ) : null}
      {status === null && !error ? (
        <div className="tg-security__loading">
          <Spinner />
        </div>
      ) : null}
      {status && view === 'overview' ? <Overview status={status} onOpen={open} /> : null}
      {status && view === 'enable' ? (
        <EnableForm token={token} onDone={(next) => done(next, t('w.settings.112678'))} onCancel={back} />
      ) : null}
      {status && view === 'change' ? (
        <ChangeForm
          token={token}
          currentHint={status.hint}
          onDone={(next) => done(next, t('w.settings.37da34'))}
          onCancel={back}
        />
      ) : null}
      {status && view === 'email' ? (
        <EmailForm
          token={token}
          pendingEmail={status.pending_recovery_email}
          onDone={(next) => done(next, t('w.settings.59561f'))}
          onCancel={back}
        />
      ) : null}
      {status && view === 'disable' ? (
        <DisableForm token={token} onDone={() => done(null, t('w.settings.ece294'))} onCancel={back} />
      ) : null}
    </section>
  )
}

function Overview({ status, onOpen }: { status: TwoFactorStatus; onOpen: (view: SecurityView) => void }) {
  if (!status.enabled) {
    return (
      <div className="tg-security__body">
        <p className="tg-security__state">{t('w.settings.a5e591')}</p>
        <Button onClick={() => onOpen('enable')} fullWidth>
          {t('w.settings.fc2484')}
        </Button>
      </div>
    )
  }
  const email = status.recovery_email
    ? status.recovery_email
    : status.pending_recovery_email
      ? t('w.settings.67c5f5', status.pending_recovery_email)
      : t('w.settings.55a04b')
  return (
    <div className="tg-security__body">
      <dl className="tg-security__facts">
        <div className="tg-security__fact">
          <dt>{t('w.settings.045859')}</dt>
          <dd>{t('w.settings.d78cde')}</dd>
        </div>
        <div className="tg-security__fact">
          <dt>{t('w.settings.b5d94a')}</dt>
          <dd>{status.hint || t('w.settings.720777')}</dd>
        </div>
        <div className="tg-security__fact">
          <dt>{t('w.settings.9f8463')}</dt>
          <dd>{email}</dd>
        </div>
      </dl>
      <div className="tg-security__actions">
        <Button variant="tonal" onClick={() => onOpen('change')} fullWidth>
          {t('w.settings.0ccab1')}
        </Button>
        <Button variant="tonal" onClick={() => onOpen('email')} fullWidth>
          {status.recovery_email ? t('w.settings.57e419') : t('w.settings.b0ed9c')}
        </Button>
        <Button variant="danger" onClick={() => onOpen('disable')} fullWidth>
          {t('w.settings.f6901f')}
        </Button>
      </div>
    </div>
  )
}
