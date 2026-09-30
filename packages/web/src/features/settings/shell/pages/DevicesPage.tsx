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
      setError('操作失败，请稍后重试')
    } finally {
      setBusy(false)
    }
  }

  if (load.state === 'loading') return <Spinner label="正在加载设备" />
  if (load.state === 'failed')
    return (
      <p className="tg-settings__error" role="alert">
        无法加载设备列表
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
            当前设备
          </h3>
          <DeviceRow session={current} now={now} />
          {others.length > 0 ? (
            confirming === 'others' ? (
              <ConfirmRow
                question="退出除本设备外的所有设备？"
                busy={busy}
                onConfirm={() => void run(() => revokeOtherDeviceSessions(apiClient, token))}
                onCancel={() => setConfirming(null)}
              />
            ) : (
              <button type="button" className="tg-settings__danger-row" onClick={() => setConfirming('others')}>
                终止所有其他会话
              </button>
            )
          ) : null}
        </section>
      ) : null}
      <section className="tg-settings__group" aria-labelledby="tg-devices-others">
        <h3 id="tg-devices-others" className="tg-settings__group-title">
          活跃会话
        </h3>
        {others.length === 0 ? <p className="tg-settings__hint">没有其他设备登录此账号</p> : null}
        <ul className="tg-settings__list">
          {others.map((session) => (
            <li key={session.id}>
              <DeviceRow session={session} now={now}>
                {confirming === session.id ? null : (
                  <Button variant="text" size="sm" onClick={() => setConfirming(session.id)}>
                    终止
                  </Button>
                )}
              </DeviceRow>
              {confirming === session.id ? (
                <ConfirmRow
                  question={`终止「${session.device_name}」的会话？`}
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
        <span className="tg-settings-device__name">{session.device_name || '未知设备'}</span>
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
        取消
      </Button>
      <Button variant="danger" size="sm" onClick={onConfirm} loading={busy}>
        终止
      </Button>
    </div>
  )
}

export default DevicesPage
