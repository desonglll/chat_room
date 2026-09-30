/**
 * One custom emoji inside text. The wrapper always carries the fallback emoji — as the
 * `data-custom-emoji` copy marker, as the accessible name, and as the visible glyph while
 * the emoji is unknown, still loading, removed, animated without a renderer, or failed.
 */
import { useState } from 'react'
import type { CustomEmoji } from '@tg/core'
import { animatedEmojiRenderer } from './animatedRenderers'
import { CUSTOM_EMOJI_ATTRIBUTE } from './copyText'
import { customEmojiServices } from './services'
import { useCachedValue } from './useCachedValue'

export interface InlineCustomEmojiProps {
  id: string
  fallback: string
  /** Pixel size; omitted = scales with the surrounding text (CSS). */
  size?: number | undefined
  className?: string | undefined
}

function Glyph({ emoji, fallback, size }: { emoji: CustomEmoji | null | undefined; fallback: string; size: number }) {
  const [failed, setFailed] = useState(false)
  if (emoji == null || failed) return <>{fallback}</>
  if (emoji.format === 'webp') {
    return (
      <img
        className="tg-custom-emoji__image"
        src={emoji.file_url}
        alt={fallback}
        width={size}
        height={size}
        loading="lazy"
        decoding="async"
        draggable={false}
        onError={() => setFailed(true)}
      />
    )
  }
  const Renderer = animatedEmojiRenderer(emoji.format)
  return Renderer ? <Renderer src={emoji.file_url} size={size} fallback={fallback} /> : <>{fallback}</>
}

export function InlineCustomEmoji({ id, fallback, size, className }: InlineCustomEmojiProps) {
  const emoji = useCachedValue(customEmojiServices().emoji, id)
  const resolved = emoji != null
  const attributes = { [CUSTOM_EMOJI_ATTRIBUTE]: fallback }
  return (
    <span
      {...attributes}
      className={`tg-custom-emoji${resolved ? ' tg-custom-emoji--resolved' : ''}${className ? ` ${className}` : ''}`}
      role="img"
      aria-label={fallback}
      style={size === undefined ? undefined : { width: size, height: size }}
    >
      <Glyph key={emoji?.file_url ?? ''} emoji={emoji} fallback={fallback} size={size ?? 20} />
    </span>
  )
}
