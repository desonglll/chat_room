/**
 * `<EmojiStatus userId />` — the custom emoji an account shows next to its name (chat
 * list row, chat header, member list). Renders nothing when the account has no status,
 * when it expired, or when the viewer may not see it (the server leaves it out).
 */
import { customEmojiServices } from './services'
import { InlineCustomEmoji } from './InlineCustomEmoji'
import { useCachedValue } from './useCachedValue'

export interface EmojiStatusProps {
  userId: string
  /** Pixel size; defaults to the name line's 18 px. */
  size?: number | undefined
}

export function EmojiStatus({ userId, size = 18 }: EmojiStatusProps) {
  const services = customEmojiServices()
  const status = useCachedValue(services.statuses, userId)
  if (status == null) return null
  if (status.expires_at !== null && Date.parse(status.expires_at) <= services.now()) return null
  return (
    <InlineCustomEmoji
      id={status.custom_emoji_id}
      fallback={status.emoji.emoji}
      size={size}
      className="tg-emoji-status"
    />
  )
}
