/**
 * The forward target picker and the delete confirmation (state: `chatDialogStore.ts`).
 * Forwarding follows Telegram Desktop: pick a chat, land in it with the forward bar on
 * its composer, add an optional comment, send.
 */
import { useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { chatListStore, composerStore } from '@tg/core'
import { Avatar, Button, Modal, TextField } from '@tg/ui'
import { useStore } from 'zustand/react'
import { closeMediaViewer } from '../mediaViewer'
import { chatDialogsStore, closeForward, settleDelete } from './chatDialogStore'
import { t } from '../../i18n/index'

export function ForwardDialog() {
  const request = useStore(chatDialogsStore, (state) => state.forward)
  const conversations = useStore(chatListStore, (state) => state.conversations)
  const navigate = useNavigate()
  const [query, setQuery] = useState('')
  const needle = query.trim().toLowerCase()
  const targets = conversations.filter((row) => {
    if (row.group?.chat_type === 'channel' && row.group.membership_role === 'member') return false
    return !needle || (row.alias || row.title).toLowerCase().includes(needle)
  })

  function pick(targetChatId: string) {
    if (!request) return
    if (request.replyTo) {
      // TG-409: a reply in the picked chat to a message of this one (a cross-chat reply).
      const source = conversations.find((row) => row.room_id === request.fromChatId)
      composerStore.getState().setReplyWithExtras(targetChatId, request.replyTo.messageId, {
        source: {
          chatId: request.fromChatId,
          chatTitle: source ? source.alias || source.title : '',
          sender: request.replyTo.sender,
          text: request.replyTo.text,
        },
      })
    } else {
      composerStore.getState().dispatchMode(targetChatId, {
        type: 'forward',
        messageIds: request.messageIds,
        fromChatId: request.fromChatId,
      })
    }
    closeForward()
    closeMediaViewer()
    setQuery('')
    void navigate(`/chat/${encodeURIComponent(targetChatId)}`)
  }

  return (
    <Modal
      open={request !== null}
      onClose={() => {
        closeForward()
        setQuery('')
      }}
      title={request?.replyTo ? t('w.chat.5744cd') : t('w.chat.3699f8')}
      size="sm"
    >
      <div className="tg-forward">
        <TextField
          label={t('w.chat.11b93c')}
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          fullWidth
        />
        <ul className="tg-forward__list" aria-label={t('w.chat.836ffe')}>
          {targets.map((row) => (
            <li key={row.room_id}>
              <button type="button" className="tg-forward__row" onClick={() => pick(row.room_id)}>
                <Avatar label={row.alias || row.title} initials={row.avatar_emoji || undefined} size="sm" />
                <span className="tg-forward__title">{row.alias || row.title}</span>
              </button>
            </li>
          ))}
          {targets.length === 0 ? <li className="tg-forward__empty">{t('w.chat.e516b3')}</li> : null}
        </ul>
      </div>
    </Modal>
  )
}

export function DeleteConfirmDialog() {
  const request = useStore(chatDialogsStore, (state) => state.remove)
  const count = request?.messageIds.length ?? 0
  return (
    <Modal
      open={request !== null}
      onClose={() => settleDelete(false)}
      title={count > 1 ? t('w.chat.b03ce1', count) : t('w.chat.6b7446')}
      description={t('w.chat.043aa8')}
      size="sm"
      footer={
        <div className="tg-confirm__actions">
          <Button variant="text" onClick={() => settleDelete(false)}>
            {t('w.chat.4d0b46')}
          </Button>
          <Button variant="danger" onClick={() => settleDelete(true)}>
            {t('w.chat.3755f5')}
          </Button>
        </div>
      }
    />
  )
}
