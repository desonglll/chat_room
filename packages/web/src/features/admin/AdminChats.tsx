/** TG-705: the global chat lock and per-chat locks for the busiest chats. */
import { useEffect, useState } from 'react'
import type { AdminApi, AdminOverview } from '@tg/core'
import { Toggle } from '@tg/ui'
import { t } from '../../i18n/index'

export function AdminChats({ api, overview }: { api: AdminApi; overview: AdminOverview }) {
  const [global, setGlobal] = useState(overview.chat_rooms_locked)
  const [locks, setLocks] = useState<Record<string, boolean>>({})
  const [error, setError] = useState('')
  useEffect(() => {
    for (const chat of overview.top_rooms) {
      api.chatLock(chat.id).then(
        (locked) => setLocks((current) => ({ ...current, [chat.id]: locked })),
        () => undefined,
      )
    }
  }, [api, overview.top_rooms])
  const fail = () => setError(t('w.admin.failed'))
  return (
    <section className="tg-admin__card" aria-label={t('w.admin.locks')}>
      <h3>{t('w.admin.locks')}</h3>
      <Toggle
        label={t('w.admin.globalLock')}
        description={t('w.admin.globalLockHint')}
        checked={global}
        onCheckedChange={(locked) => void api.setGlobalLock(locked).then(setGlobal, fail)}
      />
      <h4>{t('w.admin.topChats')}</h4>
      <ul className="tg-admin__list">
        {overview.top_rooms.map((chat) => (
          <li key={chat.id} className="tg-admin__row">
            <span className="tg-admin__grow">
              {chat.name}
              <span className="tg-admin__meta"> · {t('w.admin.chatStats', chat.messages, chat.active_members)}</span>
            </span>
            <Toggle
              label={t('w.admin.lockChat')}
              checked={locks[chat.id] ?? false}
              onCheckedChange={(locked) =>
                void api
                  .setChatLock(chat.id, locked)
                  .then((now) => setLocks((current) => ({ ...current, [chat.id]: now })), fail)
              }
            />
          </li>
        ))}
      </ul>
      {error ? <p role="alert">{error}</p> : null}
    </section>
  )
}
