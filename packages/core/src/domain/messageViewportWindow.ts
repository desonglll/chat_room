/**
 * The loaded-history window behind the virtual message list (TG-101). Pure functions over
 * chronological `BroadcastMessage[]` pages, so paging, jump-to-message joins and the
 * unread badge are unit-testable and benchmarkable without a DOM.
 *
 * Server facts these functions absorb (verified against `src/messages/store.rs`):
 * - `GET /messages?before=<id>` returns a CHRONOLOGICAL page and its cursor bound is
 *   inclusive, so the cursor message comes back again — every merge dedups by id.
 * - `GET /messages/:id/context?limit=L` returns up to `L/2 + 1` messages up to and
 *   including the target, then the newer remainder. There is no `after` cursor; asking
 *   for the context of the newest loaded message is how newer pages are fetched.
 */
import type { BroadcastMessage, DisplayMessage } from './messageView'

function byTimeThenId(left: BroadcastMessage, right: BroadcastMessage): number {
  const delta = Date.parse(left.timestamp) - Date.parse(right.timestamp)
  if (delta !== 0 && !Number.isNaN(delta)) return delta
  return left.message_id < right.message_id ? -1 : left.message_id > right.message_id ? 1 : 0
}

export interface MergeResult {
  messages: BroadcastMessage[]
  /** Messages in `page` that were not already present. */
  added: number
}

/**
 * Union of two chronological ranges, deduplicated by `message_id`; existing objects win so
 * cached layout data and React identity survive, and a page that adds nothing returns the
 * SAME array (callers may skip a state update on identity). The common case — a page
 * strictly older than everything loaded — is a concat, not a sort or a 100k-entry id set.
 */
/** First index in sorted `messages` whose entry is not less than `probe`. */
function lowerBound(messages: readonly BroadcastMessage[], probe: BroadcastMessage): number {
  let low = 0
  let high = messages.length
  while (low < high) {
    const middle = (low + high) >>> 1
    if (byTimeThenId(messages[middle] as BroadcastMessage, probe) < 0) low = middle + 1
    else high = middle
  }
  return low
}

export function mergeMessagePages(
  existing: readonly BroadcastMessage[],
  page: readonly BroadcastMessage[],
): MergeResult {
  const unique = new Map<string, BroadcastMessage>()
  for (const message of page) if (!unique.has(message.message_id)) unique.set(message.message_id, message)
  const sorted = [...unique.values()].sort(byTimeThenId)
  const first = existing[0]
  const last = existing[existing.length - 1]
  if (!first || !last) return { messages: sorted, added: sorted.length }
  // Existing is sorted and unique, so a message strictly outside [first, last] cannot be
  // in it; one inside is looked up by binary search (the inclusive `before` cursor always
  // lands here), never by building a 100k-entry id set.
  const before: BroadcastMessage[] = []
  const after: BroadcastMessage[] = []
  const inside: BroadcastMessage[] = []
  for (const message of sorted) {
    if (byTimeThenId(message, first) < 0) before.push(message)
    else if (byTimeThenId(message, last) > 0) after.push(message)
    else {
      const at = existing[lowerBound(existing, message)]
      if (!at || at.message_id !== message.message_id) inside.push(message)
    }
  }
  const added = before.length + after.length + inside.length
  if (added === 0) return { messages: existing as BroadcastMessage[], added: 0 }
  const merged = before.concat(existing, after)
  for (const message of inside) merged.splice(lowerBound(merged, message), 0, message)
  return { messages: merged, added }
}

/** Ids of the broadcast rows in a live timeline (system/upload rows have none). */
export function broadcastIds(messages: readonly DisplayMessage[]): Set<string> {
  const ids = new Set<string>()
  for (const message of messages) if (message.type === 'broadcast') ids.add(message.message_id)
  return ids
}

/** The oldest server-acknowledged message: the `before` cursor for the next older page. */
export function oldestCursor(messages: readonly DisplayMessage[]): string {
  for (const message of messages) {
    if (message.type === 'broadcast' && !message.message_id.startsWith('pending:')) return message.message_id
  }
  return ''
}

/** The newest server-acknowledged message: the context target for the next newer page. */
export function newestCursor(messages: readonly DisplayMessage[]): string {
  for (let index = messages.length - 1; index >= 0; index -= 1) {
    const message = messages[index] as DisplayMessage
    if (message.type === 'broadcast' && !message.message_id.startsWith('pending:')) return message.message_id
  }
  return ''
}

/**
 * Split a context response around its target. `reachedStart` is true when the server had
 * fewer older messages than it would have sent, i.e. the chat's first message is loaded.
 */
export function splitContextWindow(
  context: readonly BroadcastMessage[],
  targetId: string,
  limit: number,
): { targetIndex: number; reachedStart: boolean; reachedEnd: boolean } {
  const targetIndex = context.findIndex((message) => message.message_id === targetId)
  const olderLimit = Math.floor(limit / 2) + 1
  const olderCount = targetIndex + 1
  const newerCount = context.length - olderCount
  return {
    targetIndex,
    reachedStart: targetIndex >= 0 && olderCount < olderLimit,
    reachedEnd: targetIndex >= 0 && newerCount < limit - olderCount,
  }
}

/** Two contiguous ranges that share any id form one contiguous range. */
export function overlapsLive(page: readonly BroadcastMessage[], liveIds: ReadonlySet<string>): boolean {
  return page.some((message) => liveIds.has(message.message_id))
}

/** `page` minus the messages the live timeline already holds. */
export function withoutLive(page: readonly BroadcastMessage[], liveIds: ReadonlySet<string>): BroadcastMessage[] {
  return page.filter((message) => !liveIds.has(message.message_id))
}

/**
 * Virtuoso's `firstItemIndex` must drop by exactly the number of rows inserted in front of
 * the previously first row. Measured by locating that row's key in the new row list, so it
 * is correct whatever mix of prepend, hidden-lead reveal and dedup produced the change.
 */
export function prependShift(previousFirstKey: string, nextKeys: readonly string[]): number {
  if (!previousFirstKey) return 0
  const index = nextKeys.indexOf(previousFirstKey)
  return index < 0 ? 0 : index
}

/**
 * Incoming messages below the newest row the reader has seen. Scans from the end and stops
 * at the seen row, so the cost is the unread count, not the list length.
 */
export function countUnreadBelow(
  messages: readonly DisplayMessage[],
  seenKey: string,
  currentUserId: string,
  keyOf: (message: DisplayMessage) => string,
): number {
  let unread = 0
  for (let index = messages.length - 1; index >= 0; index -= 1) {
    const message = messages[index] as DisplayMessage
    if (keyOf(message) === seenKey) return unread
    if (message.type === 'broadcast' && message.sender_id !== currentUserId && !message.recalled_at) unread += 1
  }
  return seenKey ? unread : 0
}
