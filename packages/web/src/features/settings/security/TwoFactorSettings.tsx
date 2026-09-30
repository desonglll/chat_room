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
    <section className="tg-security" aria-labelledby="tg-security-title">
      <header className="tg-security__header">
        <h2 id="tg-security-title" className="tg-security__title">
          两步验证
        </h2>
        <p className="tg-security__lead">开启后，在新设备登录时除了账号密码，还需要输入你在这里设置的额外密码。</p>
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
        <EnableForm token={token} onDone={(next) => done(next, '两步验证已开启，其他设备已退出登录')} onCancel={back} />
      ) : null}
      {status && view === 'change' ? (
        <ChangeForm
          token={token}
          currentHint={status.hint}
          onDone={(next) => done(next, '两步验证已更新')}
          onCancel={back}
        />
      ) : null}
      {status && view === 'email' ? (
        <EmailForm
          token={token}
          pendingEmail={status.pending_recovery_email}
          onDone={(next) => done(next, '恢复邮箱已验证')}
          onCancel={back}
        />
      ) : null}
      {status && view === 'disable' ? (
        <DisableForm token={token} onDone={() => done(null, '两步验证已关闭')} onCancel={back} />
      ) : null}
    </section>
  )
}

function Overview({ status, onOpen }: { status: TwoFactorStatus; onOpen: (view: SecurityView) => void }) {
  if (!status.enabled) {
    return (
      <div className="tg-security__body">
        <p className="tg-security__state">当前状态：未开启</p>
        <Button onClick={() => onOpen('enable')} fullWidth>
          设置两步验证密码
        </Button>
      </div>
    )
  }
  const email = status.recovery_email
    ? status.recovery_email
    : status.pending_recovery_email
      ? `${status.pending_recovery_email}（待验证）`
      : '未设置'
  return (
    <div className="tg-security__body">
      <dl className="tg-security__facts">
        <div className="tg-security__fact">
          <dt>当前状态</dt>
          <dd>已开启</dd>
        </div>
        <div className="tg-security__fact">
          <dt>密码提示</dt>
          <dd>{status.hint || '无'}</dd>
        </div>
        <div className="tg-security__fact">
          <dt>恢复邮箱</dt>
          <dd>{email}</dd>
        </div>
      </dl>
      <div className="tg-security__actions">
        <Button variant="tonal" onClick={() => onOpen('change')} fullWidth>
          修改密码或提示
        </Button>
        <Button variant="tonal" onClick={() => onOpen('email')} fullWidth>
          {status.recovery_email ? '更换恢复邮箱' : '设置恢复邮箱'}
        </Button>
        <Button variant="danger" onClick={() => onOpen('disable')} fullWidth>
          关闭两步验证
        </Button>
      </div>
    </div>
  )
}
