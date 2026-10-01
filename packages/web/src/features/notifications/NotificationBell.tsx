/** TG-703: the bell beside chat search, with the unread count; opens `/notifications`. */
import { useEffect } from 'react'
import { useNavigate } from 'react-router-dom'
import { useStore } from 'zustand/react'
import { t } from '../../i18n/index'
import { notificationsStore, refreshUnread } from './notificationsStore'

export function NotificationBell() {
  const unread = useStore(notificationsStore, (state) => state.unread)
  const navigate = useNavigate()
  useEffect(refreshUnread, [])
  return (
    <button
      type="button"
      className="tg-bell"
      aria-label={unread > 0 ? t('w.notifications.bellUnread', unread) : t('w.notifications.title')}
      onClick={() => void navigate('/notifications')}
    >
      <svg viewBox="0 0 24 24" width="22" height="22" aria-hidden="true">
        <path
          fill="currentColor"
          d="M12 22a2.5 2.5 0 0 0 2.45-2h-4.9A2.5 2.5 0 0 0 12 22Zm7-6V11a7 7 0 0 0-5.5-6.84V3.5a1.5 1.5 0 0 0-3 0v.66A7 7 0 0 0 5 11v5l-2 2v1h18v-1l-2-2Z"
        />
      </svg>
      {unread > 0 ? <span className="tg-bell__badge">{unread > 99 ? '99+' : unread}</span> : null}
    </button>
  )
}
