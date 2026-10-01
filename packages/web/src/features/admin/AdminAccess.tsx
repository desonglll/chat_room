/** TG-705: one-time registration invites and the system administrator list (grant / revoke). */
import { useCallback, useEffect, useState } from 'react'
import type { AdminApi, SocialApi, SystemAdmin } from '@tg/core'
import { ApiError } from '@tg/core'
import { Button, TextField } from '@tg/ui'
import { t } from '../../i18n/index'

export function AdminAccess({ api, social }: { api: AdminApi; social: SocialApi }) {
  const [admins, setAdmins] = useState<SystemAdmin[]>([])
  const [hours, setHours] = useState('72')
  const [invite, setInvite] = useState<{ token: string; expires_at: string } | null>(null)
  const [username, setUsername] = useState('')
  const [note, setNote] = useState('')
  const reload = useCallback(() => api.admins().then(setAdmins, () => undefined), [api])
  useEffect(() => void reload(), [reload])

  const grant = async () => {
    const found = (await social.search(username.trim().replace(/^@/, '')).catch(() => [])).find(
      (user) => user.username.toLowerCase() === username.trim().replace(/^@/, '').toLowerCase(),
    )
    if (!found) return setNote(t('w.admin.noSuchUser'))
    await api.grant(found.id).then(
      () => {
        setUsername('')
        setNote(t('w.admin.granted', found.username))
        void reload()
      },
      () => setNote(t('w.admin.failed')),
    )
  }
  const revoke = (admin: SystemAdmin) =>
    void api.revoke(admin.user.id).then(
      () => void reload(),
      (error: unknown) =>
        setNote(error instanceof ApiError && error.status === 409 ? t('w.admin.lastAdmin') : t('w.admin.failed')),
    )

  return (
    <section className="tg-admin__card" aria-label={t('w.admin.access')}>
      <h3>{t('w.admin.access')}</h3>
      <h4>{t('w.admin.invites')}</h4>
      <div className="tg-admin__row">
        <TextField
          label={t('w.admin.inviteHours')}
          type="number"
          min={1}
          max={720}
          value={hours}
          onChange={(event) => setHours(event.target.value)}
        />
        <Button
          onClick={() => void api.createInvite(Number(hours) || 72).then(setInvite, () => setNote(t('w.admin.failed')))}
        >
          {t('w.admin.createInvite')}
        </Button>
      </div>
      {invite ? (
        <p className="tg-admin__secret">
          {t('w.admin.inviteToken')} <code>{invite.token}</code> ·{' '}
          {t('w.admin.expires', new Date(invite.expires_at).toLocaleString())}
        </p>
      ) : null}
      <h4>{t('w.admin.admins')}</h4>
      <ul className="tg-admin__list">
        {admins.map((admin) => (
          <li key={admin.user.id} className="tg-admin__row">
            <span className="tg-admin__grow">
              {admin.user.display_name || admin.user.username}{' '}
              <span className="tg-admin__meta">
                @{admin.user.username} · {admin.grant_source}
              </span>
            </span>
            <Button size="sm" variant="text" onClick={() => revoke(admin)}>
              {t('w.admin.revoke')}
            </Button>
          </li>
        ))}
      </ul>
      <form
        className="tg-admin__row"
        onSubmit={(event) => {
          event.preventDefault()
          if (username.trim()) void grant()
        }}
      >
        <TextField
          label={t('w.admin.grantUsername')}
          value={username}
          onChange={(event) => setUsername(event.target.value)}
        />
        <Button type="submit" disabled={!username.trim()}>
          {t('w.admin.grant')}
        </Button>
      </form>
      {note ? <p role="status">{note}</p> : null}
    </section>
  )
}
