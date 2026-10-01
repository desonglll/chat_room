/**
 * «联系人» (`/contacts`, TG-702; Telegram layout TG-903): a filter field and tabs over the
 * friends list (presence, online first, click to chat, actions in the row menu), friend
 * requests, the blocklist, and finding people by username. Every rule is the server's.
 */
import { useState } from 'react'
import { useNavigate } from 'react-router-dom'
import type { SocialApi } from '@tg/core'
import { t } from '../../i18n/index'
import { AddContact, BlockedList, RequestsList } from './ContactsTabs'
import { FriendsList } from './FriendsList'
import { socialApi } from './socialApi'
import { useContacts } from './useContacts'
import { useFriendStatuses } from './useFriendStatuses'
import { MobileBackButton } from '../shell/MobileBackButton'
// TG-1003: this screen is lazy, so its stylesheet travels with it, not in the first paint.
import './contacts.css'

type Tab = 'friends' | 'requests' | 'blocked' | 'add'

export function ContactsPage({ api = socialApi }: { api?: SocialApi }) {
  const contacts = useContacts(api)
  const { data, loaded, failed } = contacts
  const statuses = useFriendStatuses(api, data.friends.length)
  const [tab, setTab] = useState<Tab>('friends')
  const [filter, setFilter] = useState('')
  const [note, setNote] = useState('')
  const navigate = useNavigate()

  const act = (change: () => Promise<unknown>, done: string) =>
    contacts.act(change).then((ok) => {
      setNote(ok ? done : t('w.contacts.failed'))
      return ok
    })
  const openChat = (userId: string) =>
    void api.openPrivateChat(userId).then(
      (chatId) => void navigate(`/chat/${encodeURIComponent(chatId)}`),
      () => setNote(t('w.contacts.cannotOpen')),
    )
  const tabs: { id: Tab; label: string }[] = [
    { id: 'friends', label: t('w.contacts.friends', data.friends.length) },
    { id: 'requests', label: t('w.contacts.requests', data.incoming.length) },
    { id: 'blocked', label: t('w.contacts.blocked') },
    { id: 'add', label: t('w.contacts.add') },
  ]

  return (
    <section className="tg-contacts" aria-label={t('w.contacts.title')}>
      <header className="tg-contacts__header">
        <div className="tg-contacts__titlebar">
          <MobileBackButton />
          <h2>{t('w.contacts.title')}</h2>
        </div>
        {tab === 'friends' ? (
          <input
            className="tg-contacts__filter"
            type="search"
            aria-label={t('w.contacts.filter')}
            placeholder={t('w.contacts.filter')}
            value={filter}
            onChange={(event) => setFilter(event.target.value)}
          />
        ) : null}
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
      {note ? (
        <p className="tg-contacts__note" role="status">
          {note}
        </p>
      ) : null}
      {loaded && failed ? <p role="alert">{t('w.contacts.loadFailed')}</p> : null}

      {tab === 'friends' ? (
        <FriendsList
          friends={data.friends}
          statuses={statuses}
          query={filter}
          loaded={loaded}
          actions={{
            open: openChat,
            saveRemark: (userId, remark) => act(() => api.setRemark(userId, remark), t('w.contacts.remarkSaved')),
            remove: (userId) => void act(() => api.removeFriend(userId), t('w.contacts.removed')),
            block: (userId) => void act(() => api.block(userId), t('w.contacts.blockedDone')),
          }}
        />
      ) : null}
      {tab === 'requests' ? <RequestsList data={data} act={act} api={api} /> : null}
      {tab === 'blocked' ? <BlockedList data={data} act={act} api={api} /> : null}
      {tab === 'add' ? <AddContact act={act} api={api} openChat={openChat} /> : null}
    </section>
  )
}

export default ContactsPage
