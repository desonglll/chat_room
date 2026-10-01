/**
 * TG-508: per-chat mute durations and sound in the chat info panel. Muting for 1 h / 8 h / 2 d
 * sets `muted_until` (it lifts by itself); «永久» sets the level to `none`. The sound is a
 * per-chat exception (`/api/chats/:id/notification-exception`) over the chat type's default.
 */
import { useState } from 'react'
import { authStore, selectToken } from '@tg/core'
import { apiClient } from '../../app/client'
import { notificationSettingsApi, SOUNDS } from '../settings/notifications/notificationSettingsApi'
import { BellIcon } from './icons'

const MUTE_CHOICES: { id: string; label: string; hours: number | null }[] = [
  { id: 'unmute', label: '取消静音', hours: 0 },
  { id: '1h', label: '1 小时', hours: 1 },
  { id: '8h', label: '8 小时', hours: 8 },
  { id: '2d', label: '2 天', hours: 48 },
  { id: 'forever', label: '永久', hours: null },
]

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
  const mute = (hours: number | null, label: string) => {
    const token = selectToken(authStore.getState())
    apiClient
      .json('PATCH', `/api/conversations/${encodeURIComponent(chatId)}/preferences`, {
        ...(token ? { token } : {}),
        body: mutePatch(hours, Date.now()),
      })
      .then(
        () => setNote(hours === 0 ? '已取消静音' : `已静音：${label}`),
        () => setNote('设置失败'),
      )
  }
  return (
    <li className="tg-chatinfo__row tg-chatinfo__row--notify">
      <span className="tg-chatinfo__row-icon">
        <BellIcon />
      </span>
      <div className="tg-chatinfo__notify">
        <div className="tg-chatinfo__notify-choices" role="group" aria-label="静音时长">
          {MUTE_CHOICES.map((choice) => (
            <button key={choice.id} type="button" onClick={() => mute(choice.hours, choice.label)}>
              {choice.label}
            </button>
          ))}
        </div>
        <label className="tg-chatinfo__notify-sound">
          <span>声音</span>
          <select
            defaultValue=""
            onChange={(event) =>
              void notificationSettingsApi
                .setException(chatId, event.target.value ? { sound: event.target.value } : {})
                .then(() => setNote('已保存'))
            }
          >
            <option value="">跟随默认</option>
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
