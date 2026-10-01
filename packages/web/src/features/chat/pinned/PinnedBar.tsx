/**
 * TG-901: the bar under the chat header that shows a pinned message, as in Telegram — the
 * segmented line on the left says which of N pins is shown; a click jumps to that message and
 * moves to the next older pin; ✕ unpins it (only for those who may pin).
 */
import { useEffect, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { useStore } from 'zustand/react'
import { authStore, selectToken } from '@tg/core'
import { IconButton } from '@tg/ui'
import { apiClient } from '../../../app/client'
import { t } from '../../../i18n/index'
import { unpinMessage } from './pinnedApi'
import { nextPinIndex } from './pinnedModel'
import { pinnedStore, refreshPins, selectPins } from './pinnedStore'
import './pinned.css'

const MAX_SEGMENTS = 4

export function PinnedBar({ chatId, canUnpin }: { chatId: string; canUnpin: boolean }) {
  const pins = useStore(pinnedStore, selectPins(chatId))
  const navigate = useNavigate()
  const [index, setIndex] = useState(0)

  useEffect(() => {
    setIndex(0)
    void refreshPins(chatId)
  }, [chatId])

  if (pins.length === 0) return null
  const shown = Math.min(index, pins.length - 1)
  const pin = pins[shown]
  if (!pin) return null
  const segments = Math.min(pins.length, MAX_SEGMENTS)
  const active = Math.min(shown, segments - 1)

  return (
    <div className="tg-pinned">
      <button
        type="button"
        className="tg-pinned__body"
        onClick={() => {
          void navigate(`/chat/${encodeURIComponent(chatId)}?message=${encodeURIComponent(pin.messageId)}`)
          setIndex(nextPinIndex(shown, pins.length))
        }}
      >
        <span className="tg-pinned__track" aria-hidden="true">
          {Array.from({ length: segments }, (_, segment) => (
            <span key={segment} className="tg-pinned__segment" data-active={segment === active ? '' : undefined} />
          ))}
        </span>
        <span className="tg-pinned__text">
          <span className="tg-pinned__title">
            {pins.length > 1 ? t('w.chat.pinnedNth', shown + 1) : t('w.chat.pinnedMessage')}
          </span>
          <span className="tg-pinned__preview">{pin.text}</span>
        </span>
      </button>
      {canUnpin ? (
        <IconButton
          label={t('w.chat.unpin')}
          size="sm"
          onClick={() =>
            void unpinMessage(apiClient, selectToken(authStore.getState()), chatId, pin.messageId)
              .then(() => refreshPins(chatId))
              .catch(() => undefined)
          }
        >
          <svg
            width="18"
            height="18"
            viewBox="0 0 24 24"
            aria-hidden="true"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
          >
            <path d="M6 6l12 12M18 6 6 18" />
          </svg>
        </IconButton>
      ) : null}
    </div>
  )
}
