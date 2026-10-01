/** TG-705: deployment overview — totals, storage, runtime, service health and busiest chats. */
import type { AdminOverview } from '@tg/core'
import { formatStorageBytes } from '@tg/core'
import { t } from '../../i18n/index'

function Stat({ label, value }: { label: string; value: string | number }) {
  return (
    <div className="tg-admin__stat">
      <span className="tg-admin__stat-value">{value}</span>
      <span className="tg-admin__stat-label">{label}</span>
    </div>
  )
}

export function AdminOverviewCard({ overview }: { overview: AdminOverview }) {
  const { totals, storage, runtime } = overview
  return (
    <section className="tg-admin__card" aria-label={t('w.admin.overview')}>
      <h3>{t('w.admin.overview')}</h3>
      <div className="tg-admin__stats">
        <Stat label={t('w.admin.users')} value={totals.users} />
        <Stat label={t('w.admin.online')} value={overview.online_users} />
        <Stat label={t('w.admin.chats')} value={totals.active_rooms} />
        <Stat label={t('w.admin.messages24h')} value={totals.messages_24h} />
        <Stat label={t('w.admin.messages')} value={totals.messages} />
        <Stat label={t('w.admin.sessions')} value={totals.active_sessions} />
        <Stat label={t('w.admin.storage')} value={formatStorageBytes(storage.physical_bytes)} />
        <Stat label={t('w.admin.orphaned')} value={formatStorageBytes(storage.orphaned_bytes)} />
      </div>
      <p className="tg-admin__meta">
        {t('w.admin.backends', overview.database_backend, overview.attachment_backend)} ·{' '}
        {t(
          'w.admin.runtime',
          Math.round(runtime.uptime_seconds / 3600),
          runtime.requests,
          runtime.average_latency_ms.toFixed(1),
        )}
      </p>
      <h4>{t('w.admin.services')}</h4>
      <ul className="tg-admin__list">
        {overview.services.items.map((service) => (
          <li key={service.id} className="tg-admin__row">
            <span className="tg-admin__dot" data-state={service.state} aria-hidden="true" />
            <span className="tg-admin__grow">{service.label}</span>
            <span className="tg-admin__meta">
              {service.state}
              {service.latency_ms !== null ? ` · ${service.latency_ms} ms` : ''}
            </span>
          </li>
        ))}
      </ul>
    </section>
  )
}
