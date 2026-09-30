/**
 * TG-508 «通知与声音»: one card per chat type (on/off, message preview, sound) and the list of
 * per-chat exceptions, each removable. Precedence (server): a timed mute, then the chat's
 * exception, then the chat's level, then this default.
 */
import { useEffect, useState } from 'react'
import { Toggle } from '@tg/ui'
import {
  notificationSettingsApi,
  SCOPE_LABEL,
  SOUNDS,
  type NotificationDefaults,
  type NotificationSettings,
} from './notificationSettingsApi'

function soundLabel(id: string | null | undefined): string {
  return SOUNDS.find((sound) => sound.id === id)?.label ?? '默认'
}

export function NotificationSettingsPage() {
  const [settings, setSettings] = useState<NotificationSettings | null>(null)
  useEffect(() => {
    notificationSettingsApi.get().then(setSettings, () => setSettings({ defaults: [], exceptions: [] }))
  }, [])
  if (!settings) return null

  const saveDefaults = (next: NotificationDefaults) => {
    setSettings({ ...settings, defaults: settings.defaults.map((d) => (d.scope === next.scope ? next : d)) })
    void notificationSettingsApi.setDefaults(next).catch(() => undefined)
  }
  const removeException = (chatId: string) => {
    setSettings({ ...settings, exceptions: settings.exceptions.filter((e) => e.chat_id !== chatId) })
    void notificationSettingsApi.setException(chatId, {}).catch(() => undefined)
  }

  return (
    <div className="tg-notify-settings">
      {settings.defaults.map((defaults) => (
        <section key={defaults.scope} className="tg-notify-settings__card" aria-label={SCOPE_LABEL[defaults.scope]}>
          <h3 className="tg-notify-settings__title">{SCOPE_LABEL[defaults.scope]}</h3>
          <Toggle
            label="显示通知"
            checked={defaults.enabled}
            onCheckedChange={(enabled) => saveDefaults({ ...defaults, enabled })}
          />
          <Toggle
            label="消息预览"
            checked={defaults.preview}
            onCheckedChange={(preview) => saveDefaults({ ...defaults, preview })}
          />
          <label className="tg-notify-settings__sound">
            <span>声音</span>
            <select
              value={defaults.sound}
              onChange={(event) => saveDefaults({ ...defaults, sound: event.target.value })}
            >
              {SOUNDS.map((sound) => (
                <option key={sound.id} value={sound.id}>
                  {sound.label}
                </option>
              ))}
            </select>
          </label>
        </section>
      ))}
      <section className="tg-notify-settings__card" aria-label="例外">
        <h3 className="tg-notify-settings__title">例外</h3>
        {settings.exceptions.length === 0 ? (
          <p className="tg-notify-settings__hint">在会话信息里为单个会话设置例外。</p>
        ) : (
          <ul className="tg-notify-settings__exceptions">
            {settings.exceptions.map((exception) => (
              <li key={exception.chat_id}>
                <span>{exception.chat_title || '会话'}</span>
                <span className="tg-notify-settings__hint">
                  {exception.enabled === false ? '关闭' : exception.enabled === true ? '开启' : '默认'} ·{' '}
                  {soundLabel(exception.sound)}
                </span>
                <button type="button" onClick={() => removeException(exception.chat_id)}>
                  移除
                </button>
              </li>
            ))}
          </ul>
        )}
      </section>
    </div>
  )
}
