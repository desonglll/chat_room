/**
 * One sticker of any format, as the panel, the suggestion strip, the preview and the
 * message bubble draw it. TGS goes through TG-301's `<AnimatedSticker>` (worker rendering,
 * viewport pause, concurrency cap); WebP is a lazy `<img>`; WebM is a muted looping
 * `<video>` that does not autoplay under reduced motion.
 *
 * TG-306 is building the renderer's own format-dispatching `<Sticker>`; when it lands this
 * file becomes a one-line re-export of it — nothing else in this feature knows formats.
 */
import { usePrefersReducedMotion } from '@tg/ui'
import type { StickerFormat } from '@tg/core'
import { AnimatedSticker } from './renderer'

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
  const reduced = usePrefersReducedMotion()
  const classes = className ? `tg-sticker-view ${className}` : 'tg-sticker-view'
  if (format === 'tgs') {
    return (
      <AnimatedSticker
        src={src}
        size={size}
        autoplay={autoplay}
        loop={loop}
        {...(label === undefined ? {} : { label })}
        {...(className === undefined ? {} : { className })}
      />
    )
  }
  if (format === 'webm') {
    return (
      <video
        className={classes}
        src={src}
        width={size}
        height={size}
        muted
        playsInline
        loop={loop}
        autoPlay={autoplay && !reduced}
        preload="metadata"
        aria-label={label}
        draggable={false}
      />
    )
  }
  return (
    <img
      className={classes}
      src={src}
      width={size}
      height={size}
      alt={label ?? ''}
      loading="lazy"
      decoding="async"
      draggable={false}
    />
  )
}
