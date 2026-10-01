/**
 * TG-702 «联系人» (`/contacts`): friends (message, remark, remove, block), friend requests
 * (accept / decline incoming, cancel outgoing), the blocklist, and finding people by username
 * to send a request. Every rule (blocks, duplicates) is the server's.
 */
import { useState } from 'react'
import { useNavigate } from 'react-router-dom'
import type { SocialApi, SocialUser } from '@tg/core'
import { ApiError, contactName } from '@tg/core'
import { Avatar, Button, TextField } from '@tg/ui'
import { t } from '../../i18n/index'
import { socialApi } from './socialApi'
import { useContacts } from './useContacts'

type Tab = 'friends' | 'requests' | 'blocked' | 'add'

function Person({
  user,
  children,
}: {
  user: Pick<SocialUser, 'username' | 'display_name' | 'avatar_emoji'> & { remark?: string }
  children: React.ReactNode
}) {
  const name = contactName({ remark: user.remark ?? '', display_name: user.display_name, username: user.username })
  return (
    <li className="tg-contacts__row">
      <Avatar label={name} initials={user.avatar_emoji || undefined} size="md" />
      <span className="tg-contacts__name">
        <span>{name}</span>
        <span className="tg-contacts__handle">@{user.username}</span>
      </span>
      <span className="tg-contacts__actions">{children}</span>
    </li>
  )
}

export function ContactsPage({ api = socialApi }: { api?: SocialApi }) {
  const contacts = useContacts(api)
  const { data, loaded, failed } = contacts
  const [tab, setTab] = useState<Tab>('friends')
  const [query, setQuery] = useState('')
  const [found, setFound] = useState<SocialUser[] | null>(null)
  const [editing, setEditing] = useState<{ id: string; remark: string } | null>(null)
  const [note, setNote] = useState('')
  const navigate = useNavigate()

  const tell = (ok: boolean, done: string) => setNote(ok ? done : t('w.contacts.failed'))
  const openChat = (userId: string) =>
    api.openPrivateChat(userId).then(
      (chatId) => void navigate(`/chat/${encodeURIComponent(chatId)}`),
      () => setNote(t('w.contacts.cannotOpen')),
    )
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
  /** Every change reloads the lists and, when results are shown, the search too. */
  const act = (change: () => Promise<unknown>) =>
    contacts.act(change).then((ok) => {
      if (ok && found) search()
      return ok
    })
  const tabs: { id: Tab; label: string }[] = [
    { id: 'friends', label: t('w.contacts.friends', data.friends.length) },
    { id: 'requests', label: t('w.contacts.requests', data.incoming.length) },
    { id: 'blocked', label: t('w.contacts.blocked') },
    { id: 'add', label: t('w.contacts.add') },
  ]

  return (
    <section className="tg-contacts" aria-label={t('w.contacts.title')}>
      <header className="tg-contacts__header">
        <h2>{t('w.contacts.title')}</h2>
        <div className="tg-contacts__tabs" role="tablist">
          {tabs.map((item) => (
            <button
              key={item.id}
              type="button"
              role="tab"
              aria-selected={tab === item.id}
              onClick={() => setTab(item.id)}
            >
              {item.label}
            </button>
          ))}
        </div>
      </header>
      {note ? <p role="status">{note}</p> : null}
      {loaded && failed ? <p role="alert">{t('w.contacts.loadFailed')}</p> : null}

      {tab === 'friends' ? (
        <ul className="tg-contacts__list">
          {loaded && data.friends.length === 0 ? (
            <li className="tg-contacts__empty">{t('w.contacts.noFriends')}</li>
          ) : null}
          {data.friends.map((friend) => (
            <Person key={friend.id} user={friend}>
              {editing?.id === friend.id ? (
                <form
                  onKeyDown={(event) => {
                    if (event.key === 'Escape') setEditing(null)
                  }}
                  onSubmit={(event) => {
                    event.preventDefault()
                    void act(() => api.setRemark(friend.id, editing.remark.trim())).then((ok) => {
                      tell(ok, t('w.contacts.remarkSaved'))
                      setEditing(null)
                    })
                  }}
                >
                  <TextField
                    aria-label={t('w.contacts.remark')}
                    value={editing.remark}
                    maxLength={64}
                    onChange={(event) => setEditing({ id: friend.id, remark: event.target.value })}
                  />
                </form>
              ) : (
                <>
                  <Button size="sm" onClick={() => void openChat(friend.id)}>
                    {t('w.contacts.message')}
                  </Button>
                  <Button size="sm" variant="text" onClick={() => setEditing({ id: friend.id, remark: friend.remark })}>
                    {t('w.contacts.remark')}
                  </Button>
                  <Button
                    size="sm"
                    variant="text"
                    onClick={() =>
                      void act(() => api.removeFriend(friend.id)).then((ok) => tell(ok, t('w.contacts.removed')))
                    }
                  >
                    {t('w.contacts.remove')}
                  </Button>
                  <Button
                    size="sm"
                    variant="danger"
                    onClick={() =>
                      void act(() => api.block(friend.id)).then((ok) => tell(ok, t('w.contacts.blockedDone')))
                    }
                  >
                    {t('w.contacts.block')}
                  </Button>
                </>
              )}
            </Person>
          ))}
        </ul>
      ) : null}

      {tab === 'requests' ? (
        <ul className="tg-contacts__list">
          {data.incoming.length + data.outgoing.length === 0 ? (
            <li className="tg-contacts__empty">{t('w.contacts.noRequests')}</li>
          ) : null}
          {data.incoming.map((request) => (
            <Person key={`in-${request.user.id}`} user={request.user}>
              <Button
                size="sm"
                onClick={() =>
                  void act(() => api.respond(request.user.id, true)).then((ok) => tell(ok, t('w.contacts.accepted')))
                }
              >
                {t('w.contacts.accept')}
              </Button>
              <Button
                size="sm"
                variant="text"
                onClick={() =>
                  void act(() => api.respond(request.user.id, false)).then((ok) => tell(ok, t('w.contacts.declined')))
                }
              >
                {t('w.contacts.decline')}
              </Button>
            </Person>
          ))}
          {data.outgoing.map((request) => (
            <Person key={`out-${request.user.id}`} user={request.user}>
              <span className="tg-contacts__handle">{t('w.contacts.waiting')}</span>
              <Button
                size="sm"
                variant="text"
                onClick={() =>
                  void act(() => api.cancelRequest(request.user.id)).then((ok) => tell(ok, t('w.contacts.withdrawn')))
                }
              >
                {t('w.contacts.cancel')}
              </Button>
            </Person>
          ))}
        </ul>
      ) : null}

      {tab === 'blocked' ? (
        <ul className="tg-contacts__list">
          {data.blocked.length === 0 ? <li className="tg-contacts__empty">{t('w.contacts.noBlocked')}</li> : null}
          {data.blocked.map((user) => (
            <Person key={user.id} user={user}>
              <Button
                size="sm"
                variant="text"
                onClick={() => void act(() => api.unblock(user.id)).then((ok) => tell(ok, t('w.contacts.unblocked')))}
              >
                {t('w.contacts.unblock')}
              </Button>
            </Person>
          ))}
        </ul>
      ) : null}

      {tab === 'add' ? (
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
              <Person key={user.id} user={user}>
                {user.relationship === 'friend' ? (
                  <Button size="sm" onClick={() => void openChat(user.id)}>
                    {t('w.contacts.message')}
                  </Button>
                ) : user.relationship === 'outgoing' ? (
                  <span className="tg-contacts__handle">{t('w.contacts.waiting')}</span>
                ) : (
                  <Button
                    size="sm"
                    onClick={() =>
                      void act(() => api.sendRequest(user.id)).then((ok) =>
                        tell(
                          ok,
                          user.relationship === 'incoming' ? t('w.contacts.accepted') : t('w.contacts.requestSent'),
                        ),
                      )
                    }
                  >
                    {user.relationship === 'incoming' ? t('w.contacts.accept') : t('w.contacts.addFriend')}
                  </Button>
                )}
                <Button
                  size="sm"
                  variant="text"
                  onClick={() => void act(() => api.block(user.id)).then((ok) => tell(ok, t('w.contacts.blockedDone')))}
                >
                  {t('w.contacts.block')}
                </Button>
              </Person>
            ))}
          </ul>
        </div>
      ) : null}
    </section>
  )
}

export default ContactsPage
