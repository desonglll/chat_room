/**
 * TG-207: the admin's slow-mode interval (Telegram: Permissions › Slow Mode). Only holders of
 * `members.ban` see it; choosing an interval in a group turns it into a supergroup.
 */
import { useEffect, useState } from 'react'
import { SLOW_MODE_CHOICES, slowModeLabel } from '@tg/core'
import { slowModeApi } from './useSlowMode'
import { t } from '../../../i18n/index'

export function SlowModePicker({ chatId, api = slowModeApi }: { chatId: string; api?: typeof slowModeApi }) {
  const [seconds, setSeconds] = useState(0)
  const [busy, setBusy] = useState(true)
  const [error, setError] = useState('')

  useEffect(() => {
    let cancelled = false
    setBusy(true)
    api
      .get(chatId)
      .then((state) => {
        if (!cancelled) setSeconds(state.seconds)
      })
      .catch(() => {
        if (!cancelled) setError(t('w.chatAdmin.d9f607'))
      })
      .finally(() => {
        if (!cancelled) setBusy(false)
      })
    return () => {
      cancelled = true
    }
  }, [api, chatId])

  const choose = (next: number) => {
    if (busy || next === seconds) return
    const previous = seconds
    setSeconds(next)
    setBusy(true)
    setError('')
    api
      .set(chatId, next)
      .catch(() => {
        setSeconds(previous)
        setError(t('w.chatAdmin.ae0b2d'))
      })
      .finally(() => setBusy(false))
  }

  return (
    <section className="tg-slowmode-picker" aria-label={t('w.chatAdmin.3e5ff2')}>
      <h3 className="tg-slowmode-picker__title">{t('w.chatAdmin.3e5ff2')}</h3>
      <div className="tg-slowmode-picker__choices" role="radiogroup" aria-label={t('w.chatAdmin.8fcd17')}>
        {SLOW_MODE_CHOICES.map((choice) => (
          <button
            key={choice}
            type="button"
            role="radio"
            aria-checked={choice === seconds}
            className="tg-slowmode-picker__choice"
            disabled={busy}
            onClick={() => choose(choice)}
          >
            {slowModeLabel(choice)}
          </button>
        ))}
      </div>
      <p className="tg-slowmode-picker__note">
        {seconds > 0 ? t('w.chatAdmin.7ebf6a', slowModeLabel(seconds)) : t('w.chatAdmin.8b40ea')}
      </p>
      {error ? (
        <p className="tg-slowmode-picker__error" role="alert">
          {error}
        </p>
      ) : null}
    </section>
  )
}
