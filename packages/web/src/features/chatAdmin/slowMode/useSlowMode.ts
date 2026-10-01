/**
 * TG-207: the viewer's slow-mode countdown for one chat. The server's state (interval,
 * remaining wait, exemption) is read when the chat opens; after that the countdown restarts
 * locally whenever one of the viewer's own messages lands in the timeline, as Telegram's
 * clients do. The server stays the authority — a send inside the interval is refused there.
 */
import { useEffect, useState } from 'react'
import type { SlowModeState } from '@tg/core'
import { createSlowModeApi, messageStore, authStore, selectToken } from '@tg/core'
import { apiClient } from '../../../app/client'

export const slowModeApi = createSlowModeApi(apiClient, () => selectToken(authStore.getState()) || null)

export interface SlowModeView {
  seconds: number
  exempt: boolean
  /** Whole seconds left; 0 = may send. */
  wait: number
}

const OFF: SlowModeView = { seconds: 0, exempt: false, wait: 0 }

/**
 * Whole seconds left before the viewer may send. A chat without slow mode, or an exempt
 * viewer, never waits — whatever the clock says. `now` may lag `deadline` by a render.
 */
export function slowModeWait(state: SlowModeState | null, deadline: number, now: number): number {
  if (!state || state.exempt || state.seconds <= 0) return 0
  return Math.max(0, Math.ceil((deadline - now) / 1000))
}

/** Newest timestamp (ms) of the viewer's own server-acknowledged message in the chat. */
function lastOwnSend(chatId: string, userId: string): number {
  const messages = messageStore.getState().timelines[chatId]?.messages ?? []
  for (let index = messages.length - 1; index >= 0; index -= 1) {
    const message = messages[index]!
    if (message.type === 'broadcast' && message.sender_id === userId && !message.message_id.startsWith('pending:')) {
      return Date.parse(message.timestamp)
    }
  }
  return 0
}

export function useSlowMode(chatId: string, userId: string, api = slowModeApi): SlowModeView {
  const [state, setState] = useState<SlowModeState | null>(null)
  const [deadline, setDeadline] = useState(0)
  const [now, setNow] = useState(() => Date.now())

  useEffect(() => {
    let cancelled = false
    setState(null)
    setDeadline(0)
    api
      .get(chatId)
      .then((loaded) => {
        if (cancelled) return
        setState(loaded)
        setDeadline(Date.now() + loaded.wait_seconds * 1000)
      })
      .catch(() => {
        if (!cancelled) setState({ seconds: 0, wait_seconds: 0, exempt: false })
      })
    return () => {
      cancelled = true
    }
  }, [api, chatId])

  useEffect(() => {
    if (!state || state.exempt || state.seconds <= 0) return
    let seen = lastOwnSend(chatId, userId)
    return messageStore.subscribe(() => {
      const latest = lastOwnSend(chatId, userId)
      if (latest > seen) {
        seen = latest
        setDeadline(latest + state.seconds * 1000)
      }
    })
  }, [chatId, state, userId])

  useEffect(() => {
    setNow(Date.now())
    if (deadline <= Date.now()) return
    const timer = setInterval(() => setNow(Date.now()), 1000)
    return () => clearInterval(timer)
  }, [deadline])

  if (!state) return OFF
  return { seconds: state.seconds, exempt: state.exempt, wait: slowModeWait(state, deadline, now) }
}
