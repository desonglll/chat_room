/**
 * One sticker, as the panel, the suggestion strip, the preview and the message bubble draw
 * it: a thin adapter from this feature's `(src, format)` pair onto TG-306's format-dispatching
 * `<Sticker>` (TGS worker rendering, WebP `<img>`, WebM `<video>` with its Safari fallback,
 * all under the renderer's viewport / reduced-motion / cap policy).
 */
import { useMemo } from 'react'
import type { StickerFormat } from '@tg/core'
import { Sticker, type StickerDescriptor } from './renderer'

export interface StickerViewProps {
  src: string
  format: StickerFormat
  size: number
  /** Accessible name, usually the emoji. */
  label?: string | undefined
  autoplay?: boolean | undefined
  loop?: boolean | undefined
  className?: string | undefined
}

export function StickerView({ src, format, size, label, autoplay = true, loop = true, className }: StickerViewProps) {
  const sticker = useMemo<StickerDescriptor>(
    () => ({ format, file_url: src, emoji: label ?? null }),
    [format, src, label],
  )
  return <Sticker sticker={sticker} size={size} label={label} autoplay={autoplay} loop={loop} className={className} />
}
