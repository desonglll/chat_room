/**
 * TG-207: the admin's slow-mode interval (Telegram: Permissions › Slow Mode). Only holders of
 * `members.ban` see it; choosing an interval in a group turns it into a supergroup.
 */
import { useEffect, useState } from 'react'
import { SLOW_MODE_CHOICES, slowModeLabel } from '@tg/core'
import { slowModeApi } from './useSlowMode'

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
        if (!cancelled) setError('读取失败')
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
        setError('设置失败，请重试')
      })
      .finally(() => setBusy(false))
  }

  return (
    <section className="tg-slowmode-picker" aria-label="慢速模式">
      <h3 className="tg-slowmode-picker__title">慢速模式</h3>
      <div className="tg-slowmode-picker__choices" role="radiogroup" aria-label="发言间隔">
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
        {seconds > 0
          ? `成员每 ${slowModeLabel(seconds)}只能发送一条消息，管理员不受限制。`
          : '开启后，成员在两次发言之间需要等待。'}
      </p>
      {error ? (
        <p className="tg-slowmode-picker__error" role="alert">
          {error}
        </p>
      ) : null}
    </section>
  )
}
