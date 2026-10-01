/**
 * TG-705 «管理后台» (`/admin`): system administrators only (the server refuses everyone else
 * with 403 and this page then says so). Overview, chat locks, invites and administrators, and
 * the retention purge. AI governance stays off with the AI features.
 */
import { useCallback, useEffect, useState } from 'react'
import type { AdminApi, AdminOverview, PurgeResult, SocialApi } from '@tg/core'
import { formatStorageBytes } from '@tg/core'
import { Button } from '@tg/ui'
import { t } from '../../i18n/index'
import { socialApi } from '../contacts/socialApi'
import { AdminAccess } from './AdminAccess'
import { adminApi } from './adminApi'
import { AdminChats } from './AdminChats'
import { AdminOverviewCard } from './AdminOverviewCard'
import { MobileBackButton } from '../shell/MobileBackButton'

export function AdminPage({ api = adminApi, social = socialApi }: { api?: AdminApi; social?: SocialApi }) {
  const [overview, setOverview] = useState<AdminOverview | null>(null)
  const [denied, setDenied] = useState(false)
  const [confirmPurge, setConfirmPurge] = useState(false)
  const [purged, setPurged] = useState<PurgeResult | null>(null)
  const [error, setError] = useState('')
  const load = useCallback(
    () =>
      api.isAdmin().then(
        (ok) => (ok ? api.overview().then(setOverview) : setDenied(true)),
        () => setError(t('w.admin.failed')),
      ),
    [api],
  )
  useEffect(() => void load(), [load])

  if (denied) return <p className="tg-admin__empty">{t('w.admin.denied')}</p>
  return (
    <section className="tg-admin" aria-label={t('w.admin.title')}>
      <header className="tg-admin__header">
        <MobileBackButton />
        <h2>{t('w.admin.title')}</h2>
        <Button size="sm" variant="text" onClick={() => void load()}>
          {t('w.admin.refresh')}
        </Button>
      </header>
      {error ? <p role="alert">{error}</p> : null}
      {overview ? (
        <>
          <AdminOverviewCard overview={overview} />
          <AdminChats api={api} overview={overview} />
          <AdminAccess api={api} social={social} />
          <section className="tg-admin__card" aria-label={t('w.admin.maintenance')}>
            <h3>{t('w.admin.maintenance')}</h3>
            <p className="tg-admin__meta">
              {t('w.admin.purgeHint', overview.orphan_retention_hours, overview.deleted_room_retention_days)}
            </p>
            {confirmPurge ? (
              <Button
                variant="danger"
                onClick={() =>
                  void api.purge().then(
                    (result) => {
                      setPurged(result)
                      setConfirmPurge(false)
                      void load()
                    },
                    () => setError(t('w.admin.failed')),
                  )
                }
              >
                {t('w.admin.purgeConfirm')}
              </Button>
            ) : (
              <Button variant="tonal" onClick={() => setConfirmPurge(true)}>
                {t('w.admin.purge')}
              </Button>
            )}
            {purged ? (
              <p role="status">
                {t(
                  'w.admin.purged',
                  purged.attachment_objects_deleted,
                  formatStorageBytes(purged.attachment_bytes_deleted),
                  purged.rooms_deleted,
                )}
              </p>
            ) : null}
          </section>
        </>
      ) : (
        <p className="tg-admin__empty">{t('w.admin.loading')}</p>
      )}
    </section>
  )
}

export default AdminPage
