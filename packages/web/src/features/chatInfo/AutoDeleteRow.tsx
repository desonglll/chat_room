/**
 * TG-405: «自动删除消息» in the chat info panel. Reads the chat's timer from the chat list
 * store (kept live by `chat_updated` frames); choosing a value applies to messages sent from
 * now on, as in Telegram. Who may change it is the server's decision (either side of a
 * private chat, `chat.info` holders elsewhere); a refusal is shown, not hidden.
 */
import { useState } from 'react'
import { useStore } from 'zustand/react'
import { authStore, chatListStore, selectToken } from '@tg/core'
import { apiClient } from '../../app/client'
import { TimerIcon } from './icons'

export const AUTO_DELETE_CHOICES = [0, 86_400, 604_800, 2_592_000] as const

export function autoDeleteLabel(seconds: number): string {
  if (seconds === 86_400) return '1 天'
  if (seconds === 604_800) return '1 周'
  if (seconds === 2_592_000) return '1 个月'
  return '关闭'
}

export function AutoDeleteRow({ chatId }: { chatId: string }) {
  const seconds = useStore(
    chatListStore,
    (state) => state.chats.find((chat) => chat.id === chatId)?.auto_delete_seconds ?? 0,
  )
  const [open, setOpen] = useState(false)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')

  const choose = (next: number) => {
    setOpen(false)
    if (next === seconds) return
    setBusy(true)
    setError('')
    const token = selectToken(authStore.getState())
    apiClient
      .json<{ auto_delete_seconds: number }>('PUT', `/api/chats/${encodeURIComponent(chatId)}/auto-delete`, {
        ...(token ? { token } : {}),
        body: { seconds: next },
      })
      .then((chat) => {
        const current = chatListStore.getState().chats.find((existing) => existing.id === chatId)
        if (current)
          chatListStore.getState().applyChatUpdated({ ...current, auto_delete_seconds: chat.auto_delete_seconds })
      })
      .catch(() => setError('没有修改自动删除的权限'))
      .finally(() => setBusy(false))
  }

  return (
    <li className="tg-chatinfo__row tg-chatinfo__row--autodelete">
      <span className="tg-chatinfo__row-icon">
        <TimerIcon />
      </span>
      <div className="tg-chatinfo__autodelete">
        <button
          type="button"
          className="tg-chatinfo__autodelete-button"
          aria-haspopup="listbox"
          aria-expanded={open}
          disabled={busy}
          onClick={() => setOpen((value) => !value)}
        >
          <span className="tg-chatinfo__row-value">自动删除消息</span>
          <span className="tg-chatinfo__row-label">{autoDeleteLabel(seconds)}</span>
        </button>
        {open ? (
          <ul className="tg-chatinfo__autodelete-choices" role="listbox" aria-label="自动删除时间">
            {AUTO_DELETE_CHOICES.map((choice) => (
              <li key={choice}>
                <button
                  type="button"
                  role="option"
                  aria-selected={choice === seconds}
                  className="tg-chatinfo__autodelete-choice"
                  onClick={() => choose(choice)}
                >
                  {autoDeleteLabel(choice)}
                </button>
              </li>
            ))}
          </ul>
        ) : null}
        {seconds > 0 ? (
          <p className="tg-chatinfo__autodelete-note">此后发送的消息将在 {autoDeleteLabel(seconds)}后自动删除</p>
        ) : null}
        {error ? (
          <p className="tg-chatinfo__autodelete-note" role="alert">
            {error}
          </p>
        ) : null}
      </div>
    </li>
  )
}
