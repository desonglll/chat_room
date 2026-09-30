/**
 * The app-wide custom emoji and emoji status caches, wired to the shared `apiClient` and
 * the live session token. Tests (and a future host) swap them with `configureCustomEmoji`.
 */
import {
  MAX_CUSTOM_EMOJI_LOOKUP,
  authStore,
  getEmojiStatuses,
  resolveCustomEmoji,
  selectToken,
  type CustomEmoji,
  type EmojiStatus,
} from '@tg/core'
import { apiClient } from '../../app/client'
import { createBatchCache, type BatchCache } from './batchCache'

export interface CustomEmojiServices {
  emoji: BatchCache<CustomEmoji>
  statuses: BatchCache<EmojiStatus>
  now(): number
}

/** Emoji files never change under an id; a status may, so it is re-checked each minute. */
const STATUS_TTL_MS = 60_000
const later = (flush: () => void) => void setTimeout(flush, 0)

function createDefaultServices(): CustomEmojiServices {
  const token = () => selectToken(authStore.getState())
  const emoji = createBatchCache<CustomEmoji>({
    load: async (ids) => new Map((await resolveCustomEmoji(apiClient, token(), ids)).map((e) => [e.id, e])),
    maxBatch: MAX_CUSTOM_EMOJI_LOOKUP,
    ttlMs: Number.POSITIVE_INFINITY,
    now: Date.now,
    schedule: later,
  })
  return {
    emoji,
    statuses: createBatchCache<EmojiStatus>({
      load: async (ids) => {
        const statuses = await getEmojiStatuses(apiClient, token(), ids)
        // Each status carries its emoji: seed it so the glyph needs no second request.
        emoji.seed(statuses.map((status) => [status.emoji.id, status.emoji]))
        return new Map(statuses.map((status) => [status.user_id, status]))
      },
      maxBatch: MAX_CUSTOM_EMOJI_LOOKUP,
      ttlMs: STATUS_TTL_MS,
      now: Date.now,
      schedule: later,
    }),
    now: Date.now,
  }
}

let services: CustomEmojiServices | null = null

export function customEmojiServices(): CustomEmojiServices {
  services ??= createDefaultServices()
  return services
}

export function configureCustomEmoji(next: CustomEmojiServices | null): void {
  services = next
}
