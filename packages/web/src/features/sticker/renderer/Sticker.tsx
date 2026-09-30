/**
 * `<Sticker sticker={…}>` — the one entry for every sticker format (TG-306).
 *
 * - `tgs` → `<AnimatedSticker>` (Lottie, rendered by the manager's engine)
 * - `webp` → `<StaticSticker>` (an `<img>`)
 * - `webm` → `<VideoSticker>` (a muted looping `<video>` under the same manager policy),
 *   or its thumbnail / emoji where VP9-alpha WebM is unsupported
 *
 * The format comes from TG-302's wire fields (`format`, then `mime_type`); when both are
 * missing or unknown the bytes' magic numbers decide (see `stickerFormat.ts`).
 */
import './sticker.css'
import { useMemo } from 'react'
import { AnimatedSticker, DEFAULT_STICKER_SIZE } from './AnimatedSticker'
import { StaticSticker } from './StaticSticker'
import { useObjectUrl, useSniffedSticker } from './stickerBytes'
import { stickerUrl, wireStickerFormat, type StickerDescriptor, type StickerFormat } from './stickerFormat'
import type { StickerRenderManager } from './stickerManager'
import { VideoSticker } from './VideoSticker'

export interface StickerProps {
  sticker: StickerDescriptor
  /** Bytes already in memory (upload preview, cache); used instead of the sticker's URL. */
  data?: Uint8Array | ArrayBuffer | undefined
  /** Stable identity for `data` (TGS parse sharing). */
  cacheKey?: string | undefined
  /** CSS pixels (square). */
  size?: number | undefined
  loop?: boolean | undefined
  autoplay?: boolean | undefined
  /** Static thumbnail; defaults to `sticker.thumbnail_url`. */
  poster?: string | undefined
  /** Accessible name; defaults to `sticker.emoji`. */
  label?: string | undefined
  className?: string | undefined
  onError?: ((error: unknown) => void) | undefined
  manager?: StickerRenderManager | undefined
  /** Force the WebM capability answer (tests, fixture page). */
  webmSupported?: boolean | undefined
}

export function Sticker(props: StickerProps) {
  const { sticker, data, cacheKey, size = DEFAULT_STICKER_SIZE, loop = true, autoplay = true } = props
  const { className, onError, manager } = props
  const label = props.label ?? sticker.emoji ?? undefined
  const poster = props.poster ?? sticker.thumbnail_url ?? undefined
  const declared = wireStickerFormat(sticker)
  const url = stickerUrl(sticker)
  const bytes = useMemo(
    () => (data === undefined ? null : data instanceof Uint8Array ? data : new Uint8Array(data)),
    [data],
  )
  const sniffInput = declared ? null : bytes ? { bytes } : url ? { url } : null
  const sniffed = useSniffedSticker(sniffInput)

  const format: StickerFormat | null = declared ?? (sniffed.status === 'ready' ? sniffed.format : null)
  const body = bytes ?? (sniffed.status === 'ready' ? sniffed.bytes : null)
  const blobUrl = useObjectUrl(body, format)
  const common = { size, label, className, onError }

  if (format === null) {
    const phase = sniffed.status === 'pending' && (url || bytes) ? 'loading' : 'error'
    return <Placeholder phase={phase} size={size} label={label} className={className} />
  }
  if (format === 'tgs') {
    const tgsProps = { ...common, loop, autoplay, poster, manager }
    if (body) return <AnimatedSticker {...tgsProps} data={body} cacheKey={cacheKey} />
    if (url) return <AnimatedSticker {...tgsProps} src={url} />
    return <Placeholder phase="error" size={size} label={label} className={className} />
  }
  const src = body ? blobUrl : url
  if (!src) return <Placeholder phase={body ? 'loading' : 'error'} size={size} label={label} className={className} />
  if (format === 'webp') return <StaticSticker {...common} src={src} />
  return (
    <VideoSticker
      {...common}
      src={src}
      loop={loop}
      autoplay={autoplay}
      poster={poster}
      manager={manager}
      supported={props.webmSupported}
    />
  )
}

function Placeholder(props: {
  phase: 'loading' | 'error'
  size: number
  label?: string | undefined
  className?: string | undefined
}) {
  return (
    <span
      className={props.className ? `tg-sticker ${props.className}` : 'tg-sticker'}
      data-phase={props.phase}
      role="img"
      aria-label={props.label}
      style={{ width: props.size, height: props.size }}
    />
  )
}
