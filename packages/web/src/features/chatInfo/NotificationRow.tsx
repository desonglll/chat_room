/**
 * TG-508: per-chat mute durations and sound in the chat info panel. Muting for 1 h / 8 h / 2 d
 * sets `muted_until` (it lifts by itself); «永久» sets the level to `none`. The sound is a
 * per-chat exception (`/api/chats/:id/notification-exception`) over the chat type's default.
 */
import { useState } from 'react'
import type { ConversationPreferences } from '@tg/core'
import { authStore, chatListStore, isConversationMuted, selectToken } from '@tg/core'
import { useStore } from 'zustand/react'
import { apiClient } from '../../app/client'
import { withPreferences } from '../chatList/archiveRules'
import { notificationSettingsApi, SOUNDS } from '../settings/notifications/notificationSettingsApi'
import { BellIcon } from './icons'
import { t } from '../../i18n/index'

const MUTE_CHOICES: { id: string; label: string; hours: number | null }[] = [
  {
    id: 'unmute',
    get label() {
      return t('w.chatInfo.59bc75')
    },
    hours: 0,
  },
  {
    id: '1h',
    get label() {
      return t('w.chatInfo.c8fb1c')
    },
    hours: 1,
  },
  {
    id: '8h',
    get label() {
      return t('w.chatInfo.759ce5')
    },
    hours: 8,
  },
  {
    id: '2d',
    get label() {
      return t('w.chatInfo.922916')
    },
    hours: 48,
  },
  {
    id: 'forever',
    get label() {
      return t('w.chatInfo.409752')
    },
    hours: null,
  },
]

/**
 * TG-1208: the choices that apply now — a muted chat offers only «取消静音», an unmuted one
 * only the durations (the walkthrough found «取消静音» offered on a chat that was not muted).
 */
export function muteChoicesFor(muted: boolean): typeof MUTE_CHOICES {
  return MUTE_CHOICES.filter((choice) => (choice.hours === 0) === muted)
}

/** The preferences patch for a mute choice (pure, for tests). */
export function mutePatch(
  hours: number | null,
  now: number,
): { notification_level: string; muted_until: string | null } {
  if (hours === null) return { notification_level: 'none', muted_until: null }
  if (hours === 0) return { notification_level: 'all', muted_until: null }
  return { notification_level: 'all', muted_until: new Date(now + hours * 3_600_000).toISOString() }
}

export function NotificationRow({ chatId }: { chatId: string }) {
  const [note, setNote] = useState('')
  const muted = useStore(chatListStore, (state) => {
    const conversation = state.conversations.find((candidate) => candidate.room_id === chatId)
    return conversation ? isConversationMuted(conversation, Date.now()) : false
  })
  const mute = (hours: number | null, label: string) => {
    const token = selectToken(authStore.getState())
    apiClient
      .json<ConversationPreferences>('PATCH', `/api/conversations/${encodeURIComponent(chatId)}/preferences`, {
        ...(token ? { token } : {}),
        body: mutePatch(hours, Date.now()),
      })
      .then(
        (saved) => {
          // The stored preferences drive the choices shown here and the list's mute icon.
          const store = chatListStore.getState()
          store.setConversations(withPreferences(store.conversations, chatId, saved))
          setNote(hours === 0 ? t('w.chatInfo.02ae7c') : t('w.chatInfo.e04e05', label))
        },
        () => setNote(t('w.chatInfo.7f77f4')),
      )
  }
  return (
    <li className="tg-chatinfo__row tg-chatinfo__row--notify">
      <span className="tg-chatinfo__row-icon">
        <BellIcon />
      </span>
      <div className="tg-chatinfo__notify">
        <div className="tg-chatinfo__notify-choices" role="group" aria-label={t('w.chatInfo.97fb2c')}>
          {muteChoicesFor(muted).map((choice) => (
            <button key={choice.id} type="button" onClick={() => mute(choice.hours, choice.label)}>
              {choice.label}
            </button>
          ))}
        </div>
        <label className="tg-chatinfo__notify-sound">
          <span>{t('w.chatInfo.ab9ff8')}</span>
          <select
            defaultValue=""
            onChange={(event) =>
              void notificationSettingsApi
                .setException(chatId, event.target.value ? { sound: event.target.value } : {})
                .then(() => setNote(t('w.chatInfo.cdfab9')))
            }
          >
            <option value="">{t('w.chatInfo.a34dc2')}</option>
            {SOUNDS.map((sound) => (
              <option key={sound.id} value={sound.id}>
                {sound.label}
              </option>
            ))}
          </select>
        </label>
        {note ? <p className="tg-chatinfo__autodelete-note">{note}</p> : null}
      </div>
    </li>
  )
}
