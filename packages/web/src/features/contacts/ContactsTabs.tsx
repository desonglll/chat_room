/**
 * The contacts page's other tabs (TG-702, restyled in TG-903): friend requests, the blocklist,
 * and finding people by username. Rows share the friends list's look; every rule is the server's.
 */
import type { ReactNode } from 'react'
import { useState } from 'react'
import type { SocialApi, SocialUser } from '@tg/core'
import { ApiError, contactName } from '@tg/core'
import { Avatar, Button, TextField } from '@tg/ui'
import { t } from '../../i18n/index'
import type { ContactsData } from './useContacts'

type Act = (change: () => Promise<unknown>, done: string) => Promise<boolean>

export function PersonRow({
  user,
  subtitle,
  children,
}: {
  user: Pick<SocialUser, 'username' | 'display_name' | 'avatar_emoji'> & { remark?: string }
  subtitle?: ReactNode
  children: ReactNode
}) {
  const name = contactName({ remark: user.remark ?? '', display_name: user.display_name, username: user.username })
  return (
    <li className="tg-contacts__item">
      <div className="tg-contacts__row">
        <span className="tg-contacts__open">
          <Avatar label={name} initials={user.avatar_emoji || undefined} size="md" />
          <span className="tg-contacts__name">
            <span className="tg-contacts__title">{name}</span>
            <span className="tg-contacts__status">{subtitle ?? `@${user.username}`}</span>
          </span>
        </span>
        <span className="tg-contacts__actions">{children}</span>
      </div>
    </li>
  )
}

export function RequestsList({ data, act, api }: { data: ContactsData; act: Act; api: SocialApi }) {
  return (
    <ul className="tg-contacts__list">
      {data.incoming.length + data.outgoing.length === 0 ? (
        <li className="tg-contacts__empty">{t('w.contacts.noRequests')}</li>
      ) : null}
      {data.incoming.map((request) => (
        <PersonRow key={`in-${request.user.id}`} user={request.user}>
          <Button
            size="sm"
            onClick={() => void act(() => api.respond(request.user.id, true), t('w.contacts.accepted'))}
          >
            {t('w.contacts.accept')}
          </Button>
          <Button
            size="sm"
            variant="text"
            onClick={() => void act(() => api.respond(request.user.id, false), t('w.contacts.declined'))}
          >
            {t('w.contacts.decline')}
          </Button>
        </PersonRow>
      ))}
      {data.outgoing.map((request) => (
        <PersonRow key={`out-${request.user.id}`} user={request.user} subtitle={t('w.contacts.waiting')}>
          <Button
            size="sm"
            variant="text"
            onClick={() => void act(() => api.cancelRequest(request.user.id), t('w.contacts.withdrawn'))}
          >
            {t('w.contacts.cancel')}
          </Button>
        </PersonRow>
      ))}
    </ul>
  )
}

export function BlockedList({ data, act, api }: { data: ContactsData; act: Act; api: SocialApi }) {
  return (
    <ul className="tg-contacts__list">
      {data.blocked.length === 0 ? <li className="tg-contacts__empty">{t('w.contacts.noBlocked')}</li> : null}
      {data.blocked.map((user) => (
        <PersonRow key={user.id} user={user}>
          <Button
            size="sm"
            variant="text"
            onClick={() => void act(() => api.unblock(user.id), t('w.contacts.unblocked'))}
          >
            {t('w.contacts.unblock')}
          </Button>
        </PersonRow>
      ))}
    </ul>
  )
}

export function AddContact({ act, api, openChat }: { act: Act; api: SocialApi; openChat: (userId: string) => void }) {
  const [query, setQuery] = useState('')
  const [found, setFound] = useState<SocialUser[] | null>(null)
  const [searchNote, setSearchNote] = useState('')
  const search = () => {
    const q = query.trim().replace(/^@/, '')
    if (!q) return
    // The server answers 400 below two characters and 429 when searching too fast; both
    // used to read as «没有找到» (TG-801).
    if ([...q].length < 2) {
      setFound(null)
      setSearchNote(t('w.contacts.tooShort'))
      return
    }
    setSearchNote('')
    api.search(q).then(setFound, (error: unknown) => {
      setFound(null)
      setSearchNote(
        error instanceof ApiError && error.status === 429 ? t('w.contacts.tooFast') : t('w.contacts.searchFailed'),
      )
    })
  }
  /** Every change reloads the lists and the results. */
  const change = (run: () => Promise<unknown>, done: string) =>
    act(run, done).then((ok) => {
      if (ok) search()
      return ok
    })
  return (
    <div className="tg-contacts__add">
      <form
        className="tg-contacts__search"
        onSubmit={(event) => {
          event.preventDefault()
          search()
        }}
      >
        <TextField
          aria-label={t('w.contacts.searchLabel')}
          placeholder={t('w.contacts.searchPlaceholder')}
          value={query}
          onChange={(event) => setQuery(event.target.value)}
        />
        <Button type="submit" disabled={!query.trim()}>
          {t('w.contacts.search')}
        </Button>
      </form>
      {searchNote ? <p role="status">{searchNote}</p> : null}
      <ul className="tg-contacts__list">
        {found && found.length === 0 ? <li className="tg-contacts__empty">{t('w.contacts.nobody')}</li> : null}
        {(found ?? []).map((user) => (
          <PersonRow key={user.id} user={user}>
            {user.relationship === 'friend' ? (
              <Button size="sm" onClick={() => openChat(user.id)}>
                {t('w.contacts.message')}
              </Button>
            ) : user.relationship === 'outgoing' ? (
              <span className="tg-contacts__hint">{t('w.contacts.waiting')}</span>
            ) : (
              <Button
                size="sm"
                onClick={() =>
                  void change(
                    () => api.sendRequest(user.id),
                    user.relationship === 'incoming' ? t('w.contacts.accepted') : t('w.contacts.requestSent'),
                  )
                }
              >
                {user.relationship === 'incoming' ? t('w.contacts.accept') : t('w.contacts.addFriend')}
              </Button>
            )}
            <Button
              size="sm"
              variant="text"
              onClick={() => void change(() => api.block(user.id), t('w.contacts.blockedDone'))}
            >
              {t('w.contacts.block')}
            </Button>
          </PersonRow>
        ))}
      </ul>
    </div>
  )
}
