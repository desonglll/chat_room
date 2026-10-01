/**
 * Settings → 设备 (TG-110): this device, then every other signed-in session with a terminate
 * action, plus «终止所有其他会话». Both destructive actions ask once inline (Telegram asks in a
 * dialog; an inline confirm keeps focus in place and needs no extra layer). Backed by the
 * existing `/api/users/me/sessions` endpoints; the current session is never offered.
 */
import { useCallback, useEffect, useState, type ReactNode } from 'react'
import type { DeviceSession } from '@tg/core'
import { authStore, listDeviceSessions, revokeDeviceSession, revokeOtherDeviceSessions, selectToken } from '@tg/core'
import { Button, Spinner } from '@tg/ui'
import { useStore } from 'zustand/react'
import { apiClient } from '../../../../app/client'
import type { SettingsPageProps } from '../settingsRegistry'
import { SettingsIcon } from '../settingsIcons'
import { deviceSubtitle, orderDeviceSessions } from './devicesModel'
import { t } from '../../../../i18n/index'

type Load = { state: 'loading' } | { state: 'failed' } | { state: 'ready'; sessions: DeviceSession[] }

export function DevicesPage(_props: SettingsPageProps) {
  const token = useStore(authStore, selectToken)
  const [load, setLoad] = useState<Load>({ state: 'loading' })
  const [confirming, setConfirming] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')

  const reload = useCallback(async () => {
    try {
      setLoad({ state: 'ready', sessions: await listDeviceSessions(apiClient, token) })
    } catch {
      setLoad({ state: 'failed' })
    }
  }, [token])

  useEffect(() => {
    void reload()
  }, [reload])

  const run = async (action: () => Promise<void>) => {
    setBusy(true)
    setError('')
    try {
      await action()
      setConfirming(null)
      await reload()
    } catch {
      setError(t('w.settings.891d39'))
    } finally {
      setBusy(false)
    }
  }

  if (load.state === 'loading') return <Spinner label={t('w.settings.92ac86')} />
  if (load.state === 'failed')
    return (
      <p className="tg-settings__error" role="alert">
        {t('w.settings.9dc84d')}
      </p>
    )

  const now = new Date()
  const { current, others } = orderDeviceSessions(load.sessions)
  return (
    <div className="tg-settings-devices">
      {error ? (
        <p className="tg-settings__error" role="alert">
          {error}
        </p>
      ) : null}
      {current ? (
        <section className="tg-settings__group" aria-labelledby="tg-devices-current">
          <h3 id="tg-devices-current" className="tg-settings__group-title">
            {t('w.settings.d426cd')}
          </h3>
          <DeviceRow session={current} now={now} />
          {others.length > 0 ? (
            confirming === 'others' ? (
              <ConfirmRow
                question={t('w.settings.eb9858')}
                busy={busy}
                onConfirm={() => void run(() => revokeOtherDeviceSessions(apiClient, token))}
                onCancel={() => setConfirming(null)}
              />
            ) : (
              <button type="button" className="tg-settings__danger-row" onClick={() => setConfirming('others')}>
                {t('w.settings.6bbab6')}
              </button>
            )
          ) : null}
        </section>
      ) : null}
      <section className="tg-settings__group" aria-labelledby="tg-devices-others">
        <h3 id="tg-devices-others" className="tg-settings__group-title">
          {t('w.settings.42a08c')}
        </h3>
        {others.length === 0 ? <p className="tg-settings__hint">{t('w.settings.ba73a8')}</p> : null}
        <ul className="tg-settings__list">
          {others.map((session) => (
            <li key={session.id}>
              <DeviceRow session={session} now={now}>
                {confirming === session.id ? null : (
                  <Button variant="text" size="sm" onClick={() => setConfirming(session.id)}>
                    {t('w.settings.2eee57')}
                  </Button>
                )}
              </DeviceRow>
              {confirming === session.id ? (
                <ConfirmRow
                  question={t('w.settings.830e60', session.device_name)}
                  busy={busy}
                  onConfirm={() => void run(() => revokeDeviceSession(apiClient, token, session.id))}
                  onCancel={() => setConfirming(null)}
                />
              ) : null}
            </li>
          ))}
        </ul>
      </section>
    </div>
  )
}

function DeviceRow({ session, now, children }: { session: DeviceSession; now: Date; children?: ReactNode }) {
  return (
    <div className="tg-settings-device">
      <span className="tg-settings-device__icon">
        <SettingsIcon name="devices" />
      </span>
      <span className="tg-settings-device__text">
        <span className="tg-settings-device__name">{session.device_name || t('w.settings.f0497d')}</span>
        <span className="tg-settings-device__meta">{deviceSubtitle(session, now)}</span>
      </span>
      {children}
    </div>
  )
}

function ConfirmRow({
  question,
  busy,
  onConfirm,
  onCancel,
}: {
  question: string
  busy: boolean
  onConfirm(): void
  onCancel(): void
}) {
  return (
    <div className="tg-settings__confirm" role="group" aria-label={question}>
      <span className="tg-settings__confirm-text">{question}</span>
      <Button variant="text" size="sm" onClick={onCancel} disabled={busy}>
        {t('w.settings.4d0b46')}
      </Button>
      <Button variant="danger" size="sm" onClick={onConfirm} loading={busy}>
        {t('w.settings.2eee57')}
      </Button>
    </div>
  )
}

export default DevicesPage
