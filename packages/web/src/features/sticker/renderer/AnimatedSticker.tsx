/**
 * `<AnimatedSticker>` — a TGS (Lottie) sticker from a URL or from bytes.
 *
 * This is the TGS leaf of the sticker renderer. TG-306's `<Sticker>` dispatches on format
 * (WebP / WebM / TGS) and renders this for TGS; the playback policy it relies on — viewport
 * pause, hidden-tab pause, reduced motion, the concurrency cap — lives in the manager, not
 * here, so the other formats can reuse it.
 *
 * What the user sees: `poster` (a server thumbnail, if any) or a skeleton, then the static
 * first frame, then the animation once the sticker is on screen and admitted. Tapping a
 * sticker that does not loop by itself plays it once.
 */
import './sticker.css'
import { useEffect, useMemo, useRef, useState } from 'react'
import { stickerRenderManager } from './browserManager'
import type { StickerRenderManager, StickerView, StickerViewState } from './stickerManager'
import type { StickerSource } from './stickerSource'

type SourceProps =
  | { src: string; data?: never; cacheKey?: never }
  | {
      data: Uint8Array | ArrayBuffer
      /** Stable identity for `data`, so equal bytes share one parse; defaults to object identity. */
      cacheKey?: string
      src?: never
    }

export type AnimatedStickerProps = SourceProps & {
  /** Rendered size in CSS pixels (square). */
  size?: number
  loop?: boolean
  autoplay?: boolean
  /** Static thumbnail shown until the first frame is ready. */
  poster?: string
  /** Accessible name — usually the sticker's emoji. */
  label?: string
  className?: string
  onError?: (error: unknown) => void
  /** Injection point for tests and the benchmark; defaults to the page-wide manager. */
  manager?: StickerRenderManager
}

export const DEFAULT_STICKER_SIZE = 160

const INITIAL: StickerViewState = { phase: 'loading', poster: null }

export function AnimatedSticker(props: AnimatedStickerProps) {
  const { src, data, cacheKey, size = DEFAULT_STICKER_SIZE, loop = true, autoplay = true } = props
  const { poster, label, className, onError, manager } = props
  const hostRef = useRef<HTMLSpanElement>(null)
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const viewRef = useRef<StickerView | null>(null)
  const errorRef = useRef(onError)
  errorRef.current = onError
  const [state, setState] = useState<StickerViewState>(INITIAL)

  const source = useMemo<StickerSource | null>(() => {
    if (src !== undefined) return { url: src }
    if (data === undefined) return null
    const bytes = data instanceof Uint8Array ? data : new Uint8Array(data)
    return cacheKey === undefined ? { bytes } : { bytes, key: cacheKey }
  }, [src, data, cacheKey])

  useEffect(() => {
    const element = hostRef.current
    const canvas = canvasRef.current
    if (!element || !canvas || !source) return
    const view = (manager ?? stickerRenderManager()).attach({
      element,
      canvas,
      source,
      size,
      loop,
      autoplay,
      onState: setState,
      onError: (error) => errorRef.current?.(error),
    })
    viewRef.current = view
    return () => {
      viewRef.current = null
      view.destroy()
      setState(INITIAL)
    }
  }, [source, size, loop, autoplay, manager])

  const still = state.poster ?? poster ?? null
  const tapToPlay = !autoplay || !loop
  return (
    <span
      ref={hostRef}
      className={className ? `tg-sticker ${className}` : 'tg-sticker'}
      data-phase={state.phase}
      role="img"
      aria-label={label}
      style={{ width: size, height: size }}
      onClick={tapToPlay ? () => viewRef.current?.replay() : undefined}
    >
      {still !== null && (
        <img className="tg-sticker__still" src={still} alt="" draggable={false} hidden={state.phase === 'live'} />
      )}
      <canvas ref={canvasRef} className="tg-sticker__canvas" hidden={state.phase !== 'live'} />
    </span>
  )
}
