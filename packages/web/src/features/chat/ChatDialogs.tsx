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
      title={request?.replyTo ? '在哪个会话中回复？' : '转发到…'}
      size="sm"
    >
      <div className="tg-forward">
        <TextField label="搜索会话" value={query} onChange={(event) => setQuery(event.target.value)} fullWidth />
        <ul className="tg-forward__list" aria-label="会话">
          {targets.map((row) => (
            <li key={row.room_id}>
              <button type="button" className="tg-forward__row" onClick={() => pick(row.room_id)}>
                <Avatar label={row.alias || row.title} initials={row.avatar_emoji || undefined} size="sm" />
                <span className="tg-forward__title">{row.alias || row.title}</span>
              </button>
            </li>
          ))}
          {targets.length === 0 ? <li className="tg-forward__empty">没有可转发的会话</li> : null}
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
      title={count > 1 ? `删除 ${count} 条消息？` : '删除消息？'}
      description="消息将为所有人撤回，此操作无法撤销。"
      size="sm"
      footer={
        <div className="tg-confirm__actions">
          <Button variant="text" onClick={() => settleDelete(false)}>
            取消
          </Button>
          <Button variant="danger" onClick={() => settleDelete(true)}>
            删除
          </Button>
        </div>
      }
    />
  )
}
