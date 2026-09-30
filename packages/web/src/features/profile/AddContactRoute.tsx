/**
 * TG-511 `/add/:username` — where a scanned profile QR code lands: the account's card and
 * «添加好友» (or «发消息» when you are already friends).
 */
import { useEffect, useState } from 'react'
import { useNavigate, useParams } from 'react-router-dom'
import { authStore, selectToken } from '@tg/core'
import { Avatar, Button } from '@tg/ui'
import { apiClient } from '../../app/client'
import { profileApi, type FoundUser } from './profileApi'

export function AddContactRoute() {
  const { username = '' } = useParams()
  const navigate = useNavigate()
  const [user, setUser] = useState<FoundUser | null | undefined>(undefined)
  const [note, setNote] = useState('')

  useEffect(() => {
    let alive = true
    profileApi.findByUsername(username).then(
      (found) => alive && setUser(found),
      () => alive && setUser(null),
    )
    return () => {
      alive = false
    }
  }, [username])

  if (user === undefined) return <div className="tg-add-contact" aria-busy="true" />
  if (user === null) {
    return (
      <div className="tg-add-contact">
        <p className="tg-add-contact__hint">找不到 @{username}</p>
      </div>
    )
  }
  const friends = user.relationship === 'friend'
  const openChat = () => {
    const token = selectToken(authStore.getState())
    apiClient
      .json<{ room_id: string }>('POST', '/api/direct-chats', {
        ...(token ? { token } : {}),
        body: { user_id: user.id },
      })
      .then((chat) => void navigate(`/chat/${encodeURIComponent(chat.room_id)}`))
      .catch(() => setNote('无法打开会话'))
  }
  return (
    <div className="tg-add-contact">
      <section className="tg-add-contact__card" aria-label={`@${user.username}`}>
        <Avatar label={user.display_name || user.username} initials={user.avatar_emoji || undefined} size={96} />
        <p className="tg-add-contact__name">{user.display_name || user.username}</p>
        <p className="tg-add-contact__hint">@{user.username}</p>
        {friends ? (
          <Button variant="filled" onClick={openChat}>
            发消息
          </Button>
        ) : (
          <Button
            variant="filled"
            onClick={() =>
              void profileApi.addFriend(user.id).then(
                () => setNote('好友申请已发送'),
                () => setNote('无法发送好友申请'),
              )
            }
          >
            添加好友
          </Button>
        )}
        {note ? (
          <p className="tg-add-contact__hint" role="status">
            {note}
          </p>
        ) : null}
      </section>
    </div>
  )
}
