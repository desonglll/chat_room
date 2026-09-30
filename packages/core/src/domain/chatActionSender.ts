/**
 * TG-107: the outbound half of chat actions — what the composer / recorder / uploader calls.
 *
 * Telegram's sender contract, adapted to TG-007's frame and our 5 s receiver TTL:
 * - the same action is (re)sent at most once per CHAT_ACTION_RESEND_MS, so a burst of
 *   keystrokes costs one frame per window;
 * - a DIFFERENT action goes out immediately (typing → recording must not wait);
 * - non-`typing` actions are held open by a keep-alive on the injected clock: a user holding
 *   the record button produces no further calls, yet receivers must keep seeing it. `typing`
 *   has no keep-alive — a user who pauses stops calling, and receivers expire the line;
 * - `cancel` sends the stop frame once, only if something was actually announced;
 * - `typing` with an empty preview IS the legacy clear (TG-007 §1: empty content + typing
 *   means stopped), so it is handled as `cancel` rather than sent as a contradiction.
 * The resend window (4 s) sits below TYPING_TTL_MS (5 s) so one late frame does not flicker
 * the receiver's line off and on.
 */
import type { ClientFrame, CoreClock, CoreTimerHandle, TypingAction } from '../types'
import type { ActiveTypingAction } from './typingSummary'

export const CHAT_ACTION_RESEND_MS = 4_000

/** Server cap on the typing preview (`src/realtime/auth.rs::normalize_typing`). */
export const CHAT_ACTION_PREVIEW_CHARS = 512

export type TypingClientFrame = Extract<ClientFrame, { type: 'typing' }>

export interface ChatActionSenderOptions {
  clock: CoreClock
  /** Deliver on the chat's socket; `false` (offline) leaves the action un-announced. */
  send(chatId: string, frame: TypingClientFrame): boolean
  resendIntervalMs?: number
}

export interface ChatActionSender {
  sendChatAction(chatId: string, action: TypingAction, preview?: string): void
  /** Drops every keep-alive without sending (socket already gone). */
  dispose(): void
}

interface ActiveAction {
  action: ActiveTypingAction
  sentAt: number
  keepAlive: CoreTimerHandle | null
  preview: string
}

export function createChatActionSender(options: ChatActionSenderOptions): ChatActionSender {
  const { clock } = options
  const interval = options.resendIntervalMs ?? CHAT_ACTION_RESEND_MS
  const active = new Map<string, ActiveAction>()

  const stopKeepAlive = (entry: ActiveAction) => {
    if (entry.keepAlive !== null) clock.clearTimeout(entry.keepAlive)
    entry.keepAlive = null
  }

  const emit = (chatId: string, action: ActiveTypingAction, preview: string): boolean =>
    options.send(chatId, { type: 'typing', content: preview.slice(0, CHAT_ACTION_PREVIEW_CHARS), action })

  const armKeepAlive = (chatId: string, entry: ActiveAction) => {
    if (entry.action === 'typing') return
    entry.keepAlive = clock.setTimeout(() => {
      entry.keepAlive = null
      if (active.get(chatId) !== entry) return
      if (!emit(chatId, entry.action, entry.preview)) {
        active.delete(chatId)
        return
      }
      entry.sentAt = clock.now()
      armKeepAlive(chatId, entry)
    }, interval)
  }

  const cancel = (chatId: string) => {
    const entry = active.get(chatId)
    if (!entry) return
    stopKeepAlive(entry)
    active.delete(chatId)
    options.send(chatId, { type: 'typing', content: '', action: 'cancel' })
  }

  return {
    sendChatAction(chatId, action, preview = '') {
      if (action === 'cancel' || (action === 'typing' && !preview)) {
        cancel(chatId)
        return
      }
      const now = clock.now()
      const current = active.get(chatId)
      if (current && current.action === action && now - current.sentAt < interval) {
        current.preview = preview
        return
      }
      if (current) stopKeepAlive(current)
      if (!emit(chatId, action, preview)) {
        active.delete(chatId)
        return
      }
      const entry: ActiveAction = { action, sentAt: now, keepAlive: null, preview }
      active.set(chatId, entry)
      armKeepAlive(chatId, entry)
    },
    dispose() {
      for (const entry of active.values()) stopKeepAlive(entry)
      active.clear()
    },
  }
}
