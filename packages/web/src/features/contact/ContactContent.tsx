/**
 * TG-410: the contact card bubble — the shared account's avatar, name and handle, with
 * «发消息» (open the private chat; the server requires friendship) and «添加好友».
 */
import { useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { authStore, selectToken } from '@tg/core'
import { Avatar, Button } from '@tg/ui'
import { apiClient } from '../../app/client'
import type { MessageContentProps } from '../message'

export function ContactContent({ message, metaSpacer }: MessageContentProps) {
  const navigate = useNavigate()
  const [note, setNote] = useState('')
  const card = message.contact
  if (!card) return null
  const self = card.user_id !== null && card.user_id === authStore.getState().session?.user.id
  const token = () => {
    const value = selectToken(authStore.getState())
    return value ? { token: value } : {}
  }
  const openChat = () => {
    if (!card.user_id) return
    apiClient
      .json<{ room_id: string }>('POST', '/api/direct-chats', { ...token(), body: { user_id: card.user_id } })
      .then((chat) => void navigate(`/chat/${encodeURIComponent(chat.room_id)}`))
      .catch(() => setNote('先加为好友才能发消息'))
  }
  const addFriend = () => {
    if (!card.user_id) return
    apiClient
      .json('POST', '/api/friend-requests', { ...token(), body: { user_id: card.user_id } })
      .then(() => setNote('好友申请已发送'))
      .catch(() => setNote('无法发送好友申请'))
  }
  return (
    <div className="tg-contact-card">
      <div className="tg-contact-card__who">
        <Avatar label={card.display_name || card.username} initials={card.avatar_emoji || undefined} size="md" />
        <span className="tg-contact-card__text">
          <span className="tg-contact-card__name">{card.display_name || card.username}</span>
          <span className="tg-contact-card__handle">@{card.username}</span>
        </span>
      </div>
      {card.user_id && !self ? (
        <div className="tg-contact-card__actions">
          <Button variant="text" onClick={openChat}>
            发消息
          </Button>
          <Button variant="text" onClick={addFriend}>
            添加好友
          </Button>
        </div>
      ) : null}
      {note ? <p className="tg-contact-card__note">{note}</p> : null}
      <span className="tg-contact-card__spacer">{metaSpacer}</span>
    </div>
  )
}
