/**
 * TG-410: «联系人» in the attach menu — pick one of your friends and send their card into
 * the chat (Telegram: Attach › Contact). The card is a snapshot of the account at send time.
 */
import { useEffect, useState } from 'react'
import { authStore, selectToken } from '@tg/core'
import { Avatar, Modal, TextField } from '@tg/ui'
import { apiClient } from '../../app/client'
import { activeTopicId } from '../forum/activeTopic'
import { contactsApi } from './contactsApi'
import { t } from '../../i18n/index'

interface Friend {
  user_id: string
  username: string
  display_name: string
  avatar_emoji: string
}

/** `GET /api/friends` (`src/social/models.rs::SocialUser`). */
interface SocialUser {
  id: string
  username: string
  avatar_emoji: string
  display_name: string
  remark: string
}

export default function ContactPickerDialog({ chatId, onClose }: { chatId: string; onClose(): void }) {
  const [friends, setFriends] = useState<Friend[] | null>(null)
  const [query, setQuery] = useState('')
  const [error, setError] = useState('')

  useEffect(() => {
    const token = selectToken(authStore.getState())
    apiClient
      .json<SocialUser[]>('GET', '/api/friends', token ? { token } : {})
      .then((rows) =>
        setFriends(
          rows.map((row) => ({
            user_id: row.id,
            username: row.username,
            display_name: row.remark || row.display_name || row.username,
            avatar_emoji: row.avatar_emoji,
          })),
        ),
      )
      .catch(() => setFriends([]))
  }, [])

  const needle = query.trim().toLowerCase()
  const shown = (friends ?? []).filter(
    (friend) =>
      !needle || friend.display_name.toLowerCase().includes(needle) || friend.username.toLowerCase().includes(needle),
  )

  const send = (friend: Friend) => {
    contactsApi
      .sendContact(chatId, friend.user_id, { topicId: activeTopicId(chatId) })
      .then(onClose, () => setError(t('w.contact.91aa48')))
  }

  return (
    <Modal open onClose={onClose} title={t('w.contact.0b13dd')} size="sm">
      <div className="tg-contact-picker">
        <TextField
          label={t('w.contact.869b5e')}
          value={query}
          onChange={(event) => setQuery(event.currentTarget.value)}
          fullWidth
        />
        {error ? (
          <p className="tg-contact-picker__note" role="alert">
            {error}
          </p>
        ) : null}
        <ul className="tg-contact-picker__list">
          {shown.map((friend) => (
            <li key={friend.user_id}>
              <button type="button" className="tg-contact-picker__row" onClick={() => send(friend)}>
                <Avatar label={friend.display_name} initials={friend.avatar_emoji || undefined} size="sm" />
                <span>{friend.display_name}</span>
                <span className="tg-contact-picker__note">@{friend.username}</span>
              </button>
            </li>
          ))}
          {friends !== null && shown.length === 0 ? (
            <li className="tg-contact-picker__note">{t('w.contact.91fb91')}</li>
          ) : null}
        </ul>
      </div>
    </Modal>
  )
}
