/**
 * TG-703 «通知» (`/notifications`): mentions, replies, friend and join requests, newest first,
 * with «全部已读» and paging. Opening one marks it read and goes to its source; a source that
 * is gone (deleted, no longer readable) says so instead.
 */
import { useCallback, useEffect, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import type { NotificationsApi, NotificationView } from '@tg/core'
import { notificationTarget } from '@tg/core'
import { Avatar, Button } from '@tg/ui'
import { t } from '../../i18n/index'
import { notificationsApi, notificationsStore } from './notificationsStore'
import { MobileBackButton } from '../shell/MobileBackButton'

const KIND_LABEL: Record<NotificationView['kind'], string> = {
  get friend_request() {
    return t('w.notifications.friendRequest')
  },
  get room_join_request() {
    return t('w.notifications.joinRequest')
  },
  get mention() {
    return t('w.notifications.mention')
  },
  get reply() {
    return t('w.notifications.reply')
  },
  get ai_run_completed() {
    return t('w.notifications.ai')
  },
}

export function NotificationsPage({ api = notificationsApi }: { api?: NotificationsApi }) {
  const [items, setItems] = useState<NotificationView[]>([])
  const [cursor, setCursor] = useState<string | null>(null)
  const [loaded, setLoaded] = useState(false)
  const [note, setNote] = useState('')
  const navigate = useNavigate()

  const load = useCallback(
    (after?: string) =>
      api.list(after).then(
        (page) => {
          setItems((current) => (after ? [...current, ...page.items] : page.items))
          setCursor(page.next_cursor)
          setLoaded(true)
        },
        () => {
          setLoaded(true)
          setNote(t('w.notifications.loadFailed'))
        },
      ),
    [api],
  )
  useEffect(() => void load(), [load])

  const open = (item: NotificationView) => {
    if (!item.read_at) {
      void api.markRead(item.id).catch(() => undefined)
      setItems((current) =>
        current.map((entry) => (entry.id === item.id ? { ...entry, read_at: new Date().toISOString() } : entry)),
      )
      notificationsStore.setState((state) => ({ unread: Math.max(0, state.unread - 1) }))
    }
    const target = notificationTarget(item)
    if (target) void navigate(target)
    else setNote(t('w.notifications.gone'))
  }
  const readAll = () =>
    void api.markAllRead().then(() => {
      setItems((current) => current.map((entry) => ({ ...entry, read_at: entry.read_at ?? new Date().toISOString() })))
      notificationsStore.setState({ unread: 0 })
    })

  return (
    <section className="tg-notifications" aria-label={t('w.notifications.title')}>
      <header className="tg-notifications__header">
        <MobileBackButton />
        <h2>{t('w.notifications.title')}</h2>
        <Button size="sm" variant="text" disabled={!items.some((item) => !item.read_at)} onClick={readAll}>
          {t('w.notifications.readAll')}
        </Button>
      </header>
      {note ? <p role="status">{note}</p> : null}
      {loaded && items.length === 0 ? <p className="tg-notifications__empty">{t('w.notifications.empty')}</p> : null}
      <ul className="tg-notifications__list">
        {items.map((item) => {
          const actor = item.actor?.display_name || item.actor?.username || ''
          return (
            <li key={item.id}>
              <button
                type="button"
                className="tg-notifications__item"
                data-unread={!item.read_at || undefined}
                onClick={() => open(item)}
              >
                <Avatar
                  label={actor || KIND_LABEL[item.kind]}
                  initials={item.actor?.avatar_emoji || undefined}
                  size="md"
                />
                <span className="tg-notifications__text">
                  <span className="tg-notifications__line">
                    <strong>{actor}</strong> {KIND_LABEL[item.kind]}
                    {item.room_name ? ` · ${item.room_name}` : ''}
                  </span>
                  {item.summary ? <span className="tg-notifications__summary">{item.summary}</span> : null}
                  <time className="tg-notifications__time" dateTime={item.created_at}>
                    {new Date(item.created_at).toLocaleString()}
                  </time>
                </span>
              </button>
            </li>
          )
        })}
      </ul>
      {cursor ? (
        <Button variant="text" onClick={() => void load(cursor)}>
          {t('w.notifications.more')}
        </Button>
      ) : null}
    </section>
  )
}

export default NotificationsPage
