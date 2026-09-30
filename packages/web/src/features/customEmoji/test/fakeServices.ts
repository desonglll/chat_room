import type { CustomEmoji, EmojiStatus } from '@tg/core'
import { createBatchCache } from '../batchCache'
import { configureCustomEmoji, type CustomEmojiServices } from '../services'

export const NOW = Date.parse('2026-10-01T12:00:00Z')

export function emojiFixture(id: string, emoji = '😺', format: CustomEmoji['format'] = 'webp'): CustomEmoji {
  return {
    id,
    set_id: 'set-1',
    set_short_name: 'cats',
    emoji,
    format,
    width: 100,
    height: 100,
    file_url: `/api/stickers/${id}/file?key=k`,
  }
}

/** Services whose loaders never answer: tests seed what they need and assert markup. */
export function installFakeServices(): CustomEmojiServices & { loads: string[][] } {
  const loads: string[][] = []
  const pending = <V>() =>
    createBatchCache<V>({
      load: (keys) => {
        loads.push(keys)
        return new Promise<Map<string, V>>(() => {})
      },
      maxBatch: 200,
      ttlMs: Number.POSITIVE_INFINITY,
      now: () => NOW,
      schedule: (flush) => flush(),
    })
  const services = {
    emoji: pending<CustomEmoji>(),
    statuses: pending<EmojiStatus>(),
    now: () => NOW,
    loads,
  }
  configureCustomEmoji(services)
  return services
}
