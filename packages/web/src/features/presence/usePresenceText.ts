/**
 * React bindings for TG-107's presence text. Each hook subscribes to the stores it reads
 * plus the ONE shared ticker, and recomputes the string on either; React re-renders a row
 * only when its string actually changes (useSyncExternalStore compares snapshots).
 */
import { useCallback, useRef, useSyncExternalStore } from 'react'
import { authStore, chatListStore, presenceStore } from '@tg/core'
import type { PresenceInputs, HeaderStatus } from './presenceText'
import { headerStatusFor, lastSeenTextFor, sameHeaderStatus, typingSummaryFor } from './presenceText'
import { presenceTicker } from './presenceTicker'

function subscribeAll(listener: () => void): () => void {
  const detach = [
    presenceStore.subscribe(listener),
    chatListStore.subscribe(listener),
    authStore.subscribe(listener),
    presenceTicker.subscribe(listener),
  ]
  return () => {
    for (const off of detach) off()
  }
}

function readInputs(): PresenceInputs {
  return {
    presence: presenceStore.getState(),
    chatList: chatListStore.getState(),
    currentUserId: authStore.getState().session?.user.id ?? '',
    now: presenceTicker.now(),
  }
}

function usePresenceSnapshot<T>(compute: (inputs: PresenceInputs) => T, same: (a: T, b: T) => boolean): T {
  const cache = useRef<{ value: T } | null>(null)
  // Latest-closure refs: getSnapshot stays one identity while chatId/userId may change.
  const computeRef = useRef(compute)
  const sameRef = useRef(same)
  computeRef.current = compute
  sameRef.current = same
  const getSnapshot = useCallback(() => {
    const next = computeRef.current(readInputs())
    if (cache.current && sameRef.current(cache.current.value, next)) return cache.current.value
    cache.current = { value: next }
    return next
  }, [])
  return useSyncExternalStore(subscribeAll, getSnapshot, getSnapshot)
}

const sameString = (a: string | null, b: string | null) => a === b

/** "A 和 B 正在输入" for groups, "正在录音" for a private chat, null when nobody is active. */
export function useTypingSummary(chatId: string): string | null {
  return usePresenceSnapshot((inputs) => typingSummaryFor(inputs, chatId)?.text ?? null, sameString)
}

export type TypingStatus = Extract<HeaderStatus, { kind: 'typing' }>

const sameTyping = (a: TypingStatus | null, b: TypingStatus | null) =>
  a === b || (a !== null && b !== null && sameHeaderStatus(a, b))

/** Text plus the shared action (drives the indicator glyph), or null. */
export function useTypingStatus(chatId: string): TypingStatus | null {
  return usePresenceSnapshot((inputs) => {
    const summary = typingSummaryFor(inputs, chatId)
    return summary ? { kind: 'typing', text: summary.text, action: summary.action } : null
  }, sameTyping)
}

/** 在线 / 刚刚上线 / N 分钟前上线 / 今天 HH:mm 上线 / … for one user, kept current by the ticker. */
export function useLastSeenText(userId: string): string {
  return usePresenceSnapshot((inputs) => lastSeenTextFor(inputs.presence.users[userId], inputs.now), sameString)
}

/** What the chat header's second line shows right now. */
export function useHeaderStatus(chatId: string): HeaderStatus {
  return usePresenceSnapshot((inputs) => headerStatusFor(inputs, chatId), sameHeaderStatus)
}
