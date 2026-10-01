/**
 * TG-903: the friends tab as Telegram's contacts list — avatar with the online dot, name, and
 * the presence line («在线» in the accent colour, else «N 分钟前上线» / privacy bucket); online
 * first, then most recently seen. A click opens the private chat; right-click / long-press (or
 * the row's ⋮) holds the rest: message, edit remark, remove, block.
 */
import { useEffect, useRef, useState } from 'react'
import type { SocialUser, UserStatusEntry } from '@tg/core'
import { formatLastSeen } from '@tg/core'
import { Avatar, ContextMenu, IconButton, Menu, TextField, type MenuItem } from '@tg/ui'
import { t } from '../../i18n/index'
import type { ContactRow } from './contactsModel'
import { contactRows } from './contactsModel'

export interface FriendActions {
  open(userId: string): void
  saveRemark(userId: string, remark: string): Promise<boolean>
  remove(userId: string): void
  block(userId: string): void
}

function useNow(intervalMs: number): number {
  const [now, setNow] = useState(() => Date.now())
  useEffect(() => {
    const timer = setInterval(() => setNow(Date.now()), intervalMs)
    return () => clearInterval(timer)
  }, [intervalMs])
  return now
}

function FriendRow({ row, now, actions }: { row: ContactRow; now: number; actions: FriendActions }) {
  const [editing, setEditing] = useState<string | null>(null)
  const [menuOpen, setMenuOpen] = useState(false)
  const more = useRef<HTMLButtonElement>(null)
  const remarkForm = useRef<HTMLFormElement>(null)
  const isEditing = editing !== null
  // The menu returns focus to its trigger as it closes; take it back once that has happened.
  useEffect(() => {
    if (!isEditing) return
    const timer = setTimeout(() => remarkForm.current?.querySelector('input')?.focus(), 80)
    return () => clearTimeout(timer)
  }, [isEditing])
  const { user } = row
  const online = row.status?.kind === 'online'
  const items: MenuItem[] = [
    { id: 'message', label: t('w.contacts.message'), onSelect: () => actions.open(user.id) },
    { id: 'remark', label: t('w.contacts.remark'), onSelect: () => setEditing(user.remark) },
    { id: 'remove', label: t('w.contacts.remove'), onSelect: () => actions.remove(user.id) },
    { id: 'block', label: t('w.contacts.block'), danger: true, onSelect: () => actions.block(user.id) },
  ]
  return (
    <li className="tg-contacts__item">
      <ContextMenu items={items} aria-label={t('w.contacts.rowMenu')} className="tg-contacts__menu-region">
        <div className="tg-contacts__row">
          <button type="button" className="tg-contacts__open" onClick={() => actions.open(user.id)}>
            <Avatar label={row.name} initials={user.avatar_emoji || undefined} size="md" online={online} />
            <span className="tg-contacts__name">
              <span className="tg-contacts__title">{row.name}</span>
              {editing === null ? (
                <span className="tg-contacts__status" data-online={online ? '' : undefined}>
                  {formatLastSeen(row.status, now)}
                </span>
              ) : null}
            </span>
          </button>
          {editing !== null ? (
            <form
              ref={remarkForm}
              className="tg-contacts__remark"
              onKeyDown={(event) => {
                if (event.key === 'Escape') setEditing(null)
              }}
              onSubmit={(event) => {
                event.preventDefault()
                void actions.saveRemark(user.id, editing.trim()).then((ok) => {
                  if (ok) setEditing(null)
                })
              }}
            >
              <TextField
                aria-label={t('w.contacts.remark')}
                placeholder={t('w.contacts.remark')}
                value={editing}
                maxLength={64}
                onChange={(event) => setEditing(event.target.value)}
              />
            </form>
          ) : (
            <IconButton
              ref={more}
              label={t('w.contacts.rowMenu')}
              size="sm"
              className="tg-contacts__more"
              onClick={() => setMenuOpen(true)}
            >
              <svg width="20" height="20" viewBox="0 0 24 24" aria-hidden="true" fill="currentColor">
                <circle cx="12" cy="5" r="1.8" />
                <circle cx="12" cy="12" r="1.8" />
                <circle cx="12" cy="19" r="1.8" />
              </svg>
            </IconButton>
          )}
          <Menu
            open={menuOpen}
            onClose={() => setMenuOpen(false)}
            anchor={more}
            items={items}
            placement="bottom-end"
            aria-label={t('w.contacts.rowMenu')}
            triggerRef={more}
          />
        </div>
      </ContextMenu>
    </li>
  )
}

export function FriendsList({
  friends,
  statuses,
  query,
  loaded,
  actions,
}: {
  friends: readonly SocialUser[]
  statuses: readonly UserStatusEntry[]
  query: string
  loaded: boolean
  actions: FriendActions
}) {
  const now = useNow(30_000)
  const rows = contactRows(friends, statuses, query)
  return (
    <ul className="tg-contacts__list">
      {loaded && friends.length === 0 ? <li className="tg-contacts__empty">{t('w.contacts.noFriends')}</li> : null}
      {friends.length > 0 && rows.length === 0 ? (
        <li className="tg-contacts__empty">{t('w.contacts.noMatch')}</li>
      ) : null}
      {rows.map((row) => (
        <FriendRow key={row.user.id} row={row} now={now} actions={actions} />
      ))}
    </ul>
  )
}
