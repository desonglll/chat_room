/**
 * Composer glue: keeps the custom emoji entities of a plain-text draft in step with its
 * text. The host calls `sync(text)` on every text change (typing, paste, deletion move or
 * drop entities), `insert(...)` when the custom tab picks an emoji, and `forSend(text)` to
 * get the trimmed text and the entities to put on the `message` frame.
 */
import { useRef } from 'react'
import { entitiesForSend, insertCustomEmoji, reconcileEntities, type EntityText, type MessageEntity } from '@tg/core'
import type { PickedCustomEmoji } from './CustomEmojiGrid'

export interface EntityDraft {
  sync(text: string): void
  insert(
    text: string,
    selection: { start: number; end: number },
    emoji: PickedCustomEmoji,
  ): EntityText & { caret: number }
  forSend(text: string): EntityText
  reset(): void
  entities(): MessageEntity[]
}

export function createEntityDraft(): EntityDraft {
  let text = ''
  let entities: MessageEntity[] = []
  return {
    sync(next) {
      entities = reconcileEntities(text, next, entities)
      text = next
    },
    insert(current, selection, emoji) {
      if (current !== text) this.sync(current)
      const result = insertCustomEmoji({ text, entities }, selection, { id: emoji.id, fallback: emoji.emoji })
      text = result.text
      entities = result.entities
      return result
    },
    forSend(current) {
      if (current !== text) this.sync(current)
      return entitiesForSend(text, entities)
    },
    reset() {
      text = ''
      entities = []
    },
    entities: () => entities,
  }
}

/** One draft tracker per mounted composer. */
export function useEntityDraft(): EntityDraft {
  const ref = useRef<EntityDraft | null>(null)
  ref.current ??= createEntityDraft()
  return ref.current
}
