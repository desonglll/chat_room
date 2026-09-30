/**
 * Draft synchronisation policy (TG-008): the client half of the cloud-draft contract.
 *
 * Pure logic with an injected clock — no real timer, no socket, no storage
 * (architecture.md section 2). The card requires a 1–2 s client debounce and the server
 * makes identical writes idempotent; this helper implements the debounce and skips writes
 * it can prove redundant, so the two halves together absorb typing bursts and retries.
 */

/** What the composer edits; the payload half of `DraftWrite` (`../api/drafts.ts`). */
export interface DraftContent {
  text: string
  reply_to_message_id?: string | null
  topic_id?: string | null
}

interface NormalizedDraft {
  text: string
  reply_to_message_id: string | null
  topic_id: string | null
}

const normalize = (draft: DraftContent): NormalizedDraft => ({
  text: draft.text,
  reply_to_message_id: draft.reply_to_message_id ?? null,
  topic_id: draft.topic_id ?? null,
})

/** Content equality — the idempotence predicate, mirroring the server's comparison. */
export const draftsEqual = (a: DraftContent, b: DraftContent): boolean => {
  const [left, right] = [normalize(a), normalize(b)]
  return (
    left.text === right.text &&
    left.reply_to_message_id === right.reply_to_message_id &&
    left.topic_id === right.topic_id
  )
}

/** The timer capability the host injects; matches the global timer functions' shape. */
export interface DraftClock {
  setTimeout(callback: () => void, delayMs: number): unknown
  clearTimeout(handle: unknown): void
}

/** The card's product requirement: debounce between 1 and 2 seconds. */
export const MIN_DEBOUNCE_MS = 1000
export const MAX_DEBOUNCE_MS = 2000
export const DEFAULT_DEBOUNCE_MS = 1500

export interface DraftSynchronizerOptions {
  clock: DraftClock
  /** Clamped into [1000, 2000]: the range is a product requirement, not a hint. */
  delayMs?: number
}

export interface DraftSynchronizer {
  /** Composer changed: (re)start the debounce, or cancel it when nothing would change. */
  update(chatId: string, draft: DraftContent): void
  /** Save pending edits now (navigate away, message sent). No chatId = every chat. */
  flush(chatId?: string): void
  /** A `draft_updated` frame or GET response arrived: adopt it as the synced state. */
  accept(chatId: string, draft: DraftContent): void
  /** Whether a debounced save is scheduled — introspection for tests and UI. */
  pending(chatId: string): boolean
  /** Cancel every timer; the instance stays usable but schedules nothing until updated. */
  dispose(): void
}

/**
 * `save` is invoked at most once per debounce window per chat, and never with content equal
 * to the last state this instance saved or accepted. A rejected save forgets that state, so
 * the next update retries instead of being skipped — the server's idempotence makes the
 * retry harmless.
 */
export function createDraftSynchronizer(
  save: (chatId: string, draft: NormalizedDraft) => void | Promise<void>,
  options: DraftSynchronizerOptions,
): DraftSynchronizer {
  const { clock } = options
  const delayMs = Math.min(MAX_DEBOUNCE_MS, Math.max(MIN_DEBOUNCE_MS, options.delayMs ?? DEFAULT_DEBOUNCE_MS))
  const timers = new Map<string, unknown>()
  const queued = new Map<string, NormalizedDraft>()
  const synced = new Map<string, NormalizedDraft>()

  const cancel = (chatId: string): void => {
    const handle = timers.get(chatId)
    if (handle !== undefined) clock.clearTimeout(handle)
    timers.delete(chatId)
    queued.delete(chatId)
  }

  const commit = (chatId: string): void => {
    const draft = queued.get(chatId)
    cancel(chatId)
    if (draft === undefined) return
    synced.set(chatId, draft)
    void Promise.resolve(save(chatId, draft)).catch(() => {
      // Forget the optimistic state only if nothing newer replaced it meanwhile.
      const current = synced.get(chatId)
      if (current && draftsEqual(current, draft)) synced.delete(chatId)
    })
  }

  return {
    update(chatId, draft) {
      const next = normalize(draft)
      const last = synced.get(chatId)
      cancel(chatId)
      if (last && draftsEqual(last, next)) return
      queued.set(chatId, next)
      timers.set(
        chatId,
        clock.setTimeout(() => commit(chatId), delayMs),
      )
    },
    flush(chatId) {
      for (const id of chatId === undefined ? [...queued.keys()] : [chatId]) commit(id)
    },
    accept(chatId, draft) {
      const remote = normalize(draft)
      synced.set(chatId, remote)
      const local = queued.get(chatId)
      // The remote device caught up with the local edit: nothing left to send. A differing
      // local edit stays queued — this keyboard wins and will overwrite on commit.
      if (local && draftsEqual(local, remote)) cancel(chatId)
    },
    pending(chatId) {
      return queued.has(chatId)
    },
    dispose() {
      for (const chatId of [...queued.keys()]) cancel(chatId)
    },
  }
}
