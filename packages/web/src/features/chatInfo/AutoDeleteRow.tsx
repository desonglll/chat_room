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
import { t } from '../../i18n/index'

export const AUTO_DELETE_CHOICES = [0, 86_400, 604_800, 2_592_000] as const

export function autoDeleteLabel(seconds: number): string {
  if (seconds === 86_400) return t('w.chatInfo.11f478')
  if (seconds === 604_800) return t('w.chatInfo.03f320')
  if (seconds === 2_592_000) return t('w.chatInfo.178802')
  return t('w.chatInfo.6c14bd')
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
      .catch(() => setError(t('w.chatInfo.2719b4')))
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
          <span className="tg-chatinfo__row-value">{t('w.chatInfo.9aafa7')}</span>
          <span className="tg-chatinfo__row-label">{autoDeleteLabel(seconds)}</span>
        </button>
        {open ? (
          <ul className="tg-chatinfo__autodelete-choices" role="listbox" aria-label={t('w.chatInfo.8d8da2')}>
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
          <p className="tg-chatinfo__autodelete-note">
            {t('w.chatInfo.4d234b')} {autoDeleteLabel(seconds)}
            {t('w.chatInfo.67f4d2')}
          </p>
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
