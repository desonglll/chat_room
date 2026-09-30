/**
 * Set or clear the viewer's emoji status: pick a duration, then a custom emoji. The own
 * status cache is updated from the server's answer, so every `<EmojiStatus>` of the
 * viewer changes at once.
 */
import { useState } from 'react'
import { authStore, clearEmojiStatus, selectToken, setEmojiStatus } from '@tg/core'
import { useStore } from 'zustand/react'
import { apiClient } from '../../app/client'
import { CustomEmojiGrid, type PickedCustomEmoji } from './CustomEmojiGrid'
import { customEmojiServices } from './services'
import { STATUS_DURATIONS, expiryFor } from './statusDurations'
import { useInstalledCustomEmoji, type InstalledLoader } from './useInstalledCustomEmoji'

export interface EmojiStatusPickerProps {
  onDone?: (() => void) | undefined
  load?: InstalledLoader | undefined
}

export function EmojiStatusPicker({ onDone, load }: EmojiStatusPickerProps) {
  const state = useInstalledCustomEmoji(load)
  const userId = useStore(authStore, (auth) => auth.session?.user.id ?? '')
  const [duration, setDuration] = useState(0)
  const [error, setError] = useState('')

  const apply = async (picked: PickedCustomEmoji | null) => {
    const token = selectToken(authStore.getState())
    const services = customEmojiServices()
    setError('')
    try {
      if (picked === null) {
        await clearEmojiStatus(apiClient, token)
        services.statuses.seed([[userId, null]])
      } else {
        const choice = STATUS_DURATIONS[duration] ?? STATUS_DURATIONS[0]!
        const status = await setEmojiStatus(apiClient, token, {
          custom_emoji_id: picked.id,
          expires_at: expiryFor(choice, services.now()),
        })
        services.statuses.seed([[userId, status]])
      }
      onDone?.()
    } catch {
      setError('设置失败，请重试')
    }
  }

  return (
    <div className="tg-emoji-status-picker">
      <div className="tg-emoji-status-picker__durations" role="radiogroup" aria-label="状态有效期">
        {STATUS_DURATIONS.map((choice, index) => (
          <button
            key={choice.label}
            type="button"
            role="radio"
            aria-checked={index === duration}
            className="tg-emoji-status-picker__duration"
            onClick={() => setDuration(index)}
          >
            {choice.label}
          </button>
        ))}
      </div>
      {state.status === 'ready' ? (
        <CustomEmojiGrid sets={state.sets} onPick={(picked) => void apply(picked)} />
      ) : (
        <p className="tg-custom-emoji-grid__empty">{state.status === 'loading' ? '加载中…' : '自定义表情加载失败'}</p>
      )}
      <button type="button" className="tg-emoji-status-picker__clear" onClick={() => void apply(null)}>
        清除状态
      </button>
      {error ? <p className="tg-emoji-status-picker__error">{error}</p> : null}
    </div>
  )
}
