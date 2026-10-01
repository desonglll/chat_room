/**
 * TG-802: a forward the server skipped for the recipient's privacy is shown under the
 * composer of the chat it was meant for, then fades on its own. Framework-free; the hook
 * below binds it to React.
 */
import { useSyncExternalStore } from 'react'
import type { ForwardResult } from '@tg/core'
import { t } from '../../i18n/index'

const NOTICE_MS = 5_000
const notices = new Map<string, string>()
const listeners = new Set<() => void>()

function emit(): void {
  for (const listener of listeners) listener()
}

/** The notice a batch of forward results earns, or null when nothing was refused by privacy. */
export function forwardRefusalNotice(results: readonly ForwardResult[]): string | null {
  return results.some((result) => result.skipped_reason === 'voice_messages_restricted') ? t('w.voice.162eb5') : null
}

export function reportForwardResults(chatId: string, results: readonly ForwardResult[]): void {
  const notice = forwardRefusalNotice(results)
  if (!notice) return
  notices.set(chatId, notice)
  emit()
  setTimeout(() => {
    if (notices.get(chatId) !== notice) return
    notices.delete(chatId)
    emit()
  }, NOTICE_MS)
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener)
  return () => listeners.delete(listener)
}

export function useForwardNotice(chatId: string): string | null {
  return useSyncExternalStore(
    subscribe,
    () => notices.get(chatId) ?? null,
    () => null,
  )
}
