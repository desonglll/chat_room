/**
 * TG-706 «操作日志»: the chat's audit events for its managers. The entry appears only after the
 * first page loads (anyone else gets 403 from the server and sees nothing).
 */
import { useEffect, useState } from 'react'
import type { AuditEvent, TasksApi } from '@tg/core'
import { Button, Sheet } from '@tg/ui'
import { t } from '../../i18n/index'
import { tasksApi } from './tasksApi'

export function AuditLogEntry({ chatId, api = tasksApi }: { chatId: string; api?: TasksApi }) {
  const [events, setEvents] = useState<AuditEvent[] | null>(null)
  const [cursor, setCursor] = useState<string | null>(null)
  const [open, setOpen] = useState(false)
  useEffect(() => {
    let cancelled = false
    api.auditEvents(chatId).then(
      (page) => {
        if (cancelled) return
        setEvents(page.items)
        setCursor(page.next_cursor)
      },
      () => undefined,
    )
    return () => {
      cancelled = true
    }
  }, [api, chatId])
  if (events === null) return null
  return (
    <>
      <Button variant="text" fullWidth onClick={() => setOpen(true)}>
        {t('w.tasks.auditLog')}
      </Button>
      <Sheet open={open} side="right" title={t('w.tasks.auditLog')} onClose={() => setOpen(false)}>
        {events.length === 0 ? <p className="tg-tasks__empty">{t('w.tasks.noEvents')}</p> : null}
        <ul className="tg-tasks__list">
          {events.map((event) => (
            <li key={event.id} className="tg-tasks__event">
              <strong>{event.actor_username}</strong> <code>{event.event_type}</code>
              {event.target_type ? <span className="tg-tasks__meta"> · {event.target_type}</span> : null}
              <time className="tg-tasks__meta" dateTime={event.created_at}>
                {' '}
                {new Date(event.created_at).toLocaleString()}
              </time>
            </li>
          ))}
        </ul>
        {cursor ? (
          <Button
            variant="text"
            onClick={() =>
              void api.auditEvents(chatId, cursor).then((page) => {
                setEvents((current) => [...(current ?? []), ...page.items])
                setCursor(page.next_cursor)
              })
            }
          >
            {t('w.tasks.more')}
          </Button>
        ) : null}
      </Sheet>
    </>
  )
}
