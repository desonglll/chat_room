/**
 * Batches "this post is on screen" into view reports. A post mounted by the virtual list is
 * reported once per session; ids collect for `delayMs` and go out in reports of at most
 * `MAX_VIEWED_POSTS`, one chat per report. The answers (counts as stored) feed the store; the
 * view itself comes back to every subscriber in the server's batched frame.
 *
 * Framework-free and injected (timer, report function) so the batching is unit-tested.
 */
import type { MessageViewCount } from '@tg/core'
import { MAX_VIEWED_POSTS } from '@tg/core'

export interface ViewReporterOptions {
  report(chatId: string, messageIds: readonly string[]): Promise<MessageViewCount[]>
  onCounts(counts: readonly MessageViewCount[]): void
  setTimer(callback: () => void, delayMs: number): unknown
  delayMs?: number
}

export interface ViewReporter {
  /** `messageId` of `chatId` is on screen. Idempotent. */
  seen(chatId: string, messageId: string): void
  /** Send what is queued now. */
  flush(): void
}

export function createViewReporter(options: ViewReporterOptions): ViewReporter {
  const delayMs = options.delayMs ?? 400
  const reported = new Set<string>()
  const queued = new Map<string, string[]>()
  let scheduled = false

  function flush(): void {
    scheduled = false
    for (const [chatId, ids] of queued) {
      for (let start = 0; start < ids.length; start += MAX_VIEWED_POSTS) {
        const batch = ids.slice(start, start + MAX_VIEWED_POSTS)
        options.report(chatId, batch).then(options.onCounts, () => {
          // A failed report is retried the next time the post mounts.
          for (const id of batch) reported.delete(`${chatId}:${id}`)
        })
      }
    }
    queued.clear()
  }

  return {
    seen(chatId, messageId) {
      if (!chatId || !messageId || messageId.startsWith('pending:')) return
      const key = `${chatId}:${messageId}`
      if (reported.has(key)) return
      reported.add(key)
      const ids = queued.get(chatId) ?? []
      ids.push(messageId)
      queued.set(chatId, ids)
      if (!scheduled) {
        scheduled = true
        options.setTimer(flush, delayMs)
      }
    },
    flush,
  }
}
