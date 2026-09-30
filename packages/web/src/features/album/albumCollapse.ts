/**
 * TG-403: an album is N ordinary messages sharing `grouped_id`; the list draws the
 * consecutive run as ONE row. `collapseAlbums` replaces each run (same `grouped_id`, same
 * sender, adjacent) with a representative: a shallow copy of the first item — its id and
 * row key, so the row key is stable while the rest of the album streams in — carrying the
 * last item's timestamp (Telegram shows the album's time once, at its end).
 *
 * The items are looked up with `albumItemsOf(representative)`, a WeakMap side table, so no
 * wire or core type grows a client-only field. Representatives are cached per `grouped_id`
 * and reused while their items are identical objects: the list's layout cache (TG-101)
 * compares by identity, so an unrelated append must not re-render every album above it.
 *
 * A lone item of an album (the others recalled, or not loaded yet) stays an ordinary row.
 */
import type { BroadcastMessage, DisplayMessage } from '@tg/core'

const itemsOf = new WeakMap<BroadcastMessage, readonly BroadcastMessage[]>()

/** The album items a representative stands for, oldest first; `undefined` for any other row. */
export function albumItemsOf(message: DisplayMessage): readonly BroadcastMessage[] | undefined {
  return message.type === 'broadcast' ? itemsOf.get(message) : undefined
}

function groupOf(message: DisplayMessage): string | undefined {
  if (message.type !== 'broadcast' || message.recalled_at) return undefined
  return message.grouped_id || undefined
}

function sameItems(a: readonly BroadcastMessage[], b: readonly BroadcastMessage[]): boolean {
  return a.length === b.length && a.every((item, index) => item === b[index])
}

export interface AlbumCollapser {
  (messages: DisplayMessage[]): DisplayMessage[]
}

export function createAlbumCollapser(): AlbumCollapser {
  let cache = new Map<string, BroadcastMessage>()
  return (messages) => {
    // Fast path: no album on screen, hand the input back untouched.
    if (!messages.some((message) => groupOf(message) !== undefined)) {
      if (cache.size > 0) cache = new Map()
      return messages
    }
    const next = new Map<string, BroadcastMessage>()
    const out: DisplayMessage[] = []
    let index = 0
    while (index < messages.length) {
      const first = messages[index] as DisplayMessage
      const group = groupOf(first)
      let end = index + 1
      if (group !== undefined) {
        const sender = (first as BroadcastMessage).sender_id
        while (end < messages.length) {
          const candidate = messages[end] as DisplayMessage
          if (groupOf(candidate) !== group || (candidate as BroadcastMessage).sender_id !== sender) break
          end += 1
        }
      }
      if (group === undefined || end - index < 2) {
        out.push(first)
        index += 1
        continue
      }
      const items = messages.slice(index, end) as BroadcastMessage[]
      const cached = cache.get(group)
      const cachedItems = cached ? itemsOf.get(cached) : undefined
      let representative: BroadcastMessage
      if (cached && cachedItems && sameItems(cachedItems, items)) {
        representative = cached
      } else {
        const last = items[items.length - 1] as BroadcastMessage
        representative = { ...(items[0] as BroadcastMessage), timestamp: last.timestamp }
        itemsOf.set(representative, items)
      }
      next.set(group, representative)
      out.push(representative)
      index = end
    }
    cache = next
    return out
  }
}

/** Every message id a row stands for: the album's items, or the message itself. */
export function rowMessageIds(message: DisplayMessage): string[] {
  const items = albumItemsOf(message)
  if (items) return items.map((item) => item.message_id)
  return message.type === 'broadcast' ? [message.message_id] : []
}
