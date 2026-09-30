/**
 * `<VideoSticker>` — a WebM (VP9 + alpha) sticker played by a muted, looping, inline `<video>`.
 *
 * Playback is decided by the sticker manager, like TGS: only on screen, never in a hidden tab,
 * inside the shared concurrency cap, and never under `prefers-reduced-motion` (then the first
 * frame is held). Where the browser cannot show VP9 alpha (`webmSupport.ts`) or the video
 * fails to decode, the sticker falls back to its thumbnail (`poster`), else to its emoji.
 */
import './sticker.css'
import { useEffect, useRef, useState, type ReactNode } from 'react'
import { stickerRenderManager } from './browserManager'
import type { MotionView } from './motionViews'
import type { StickerRenderManager } from './stickerManager'
import { webmStickerSupported } from './webmSupport'

export interface VideoStickerProps {
  src: string
  size: number
  loop: boolean
  autoplay: boolean
  poster?: string | undefined
  label?: string | undefined
  className?: string | undefined
  onError?: ((error: unknown) => void) | undefined
  manager?: StickerRenderManager | undefined
  /** Override the capability check (tests); defaults to `webmStickerSupported()`. */
  supported?: boolean | undefined
}

type Phase = 'loading' | 'static' | 'live'

export function VideoSticker(props: VideoStickerProps) {
  const { src, size, loop, autoplay, poster, label, className, onError, manager } = props
  const supported = props.supported ?? webmStickerSupported()
  const hostRef = useRef<HTMLSpanElement>(null)
  const videoRef = useRef<HTMLVideoElement>(null)
  const viewRef = useRef<MotionView | null>(null)
  const errorRef = useRef(onError)
  errorRef.current = onError
  const [phase, setPhase] = useState<Phase>('loading')
  const [failed, setFailed] = useState<string | null>(null)
  const fallback = !supported || failed === src

  useEffect(() => {
    const element = hostRef.current
    const video = videoRef.current
    if (fallback || !element || !video) return
    video.muted = true
    setPhase('loading')
    const view = (manager ?? stickerRenderManager()).attachMotion({
      element,
      key: src,
      loop,
      autoplay,
      onPlayback(playing, reducedMotion) {
        if (playing) {
          // Autoplay policy can still refuse (e.g. data saver); the still frame stays up.
          video.play().catch(() => undefined)
          return
        }
        video.pause()
        if (reducedMotion && video.currentTime !== 0) video.currentTime = 0
      },
    })
    viewRef.current = view
    return () => {
      viewRef.current = null
      view.destroy()
      video.pause()
    }
  }, [src, loop, autoplay, manager, fallback])

  const tapToPlay = !autoplay || !loop
  const host = (dataPhase: string, children: ReactNode, extra: Record<string, string> = {}) => (
    <span
      ref={hostRef}
      className={className ? `tg-sticker ${className}` : 'tg-sticker'}
      data-phase={dataPhase}
      data-format="webm"
      {...extra}
      role="img"
      aria-label={label}
      style={{ width: size, height: size }}
      onClick={tapToPlay && !fallback ? () => viewRef.current?.replay() : undefined}
    >
      {children}
    </span>
  )

  if (fallback)
    return host(
      'static',
      poster ? (
        <img className="tg-sticker__still" src={poster} alt="" draggable={false} />
      ) : (
        <span className="tg-sticker__emoji" aria-hidden="true" style={{ fontSize: Math.round(size * 0.6) }}>
          {label}
        </span>
      ),
      { 'data-fallback': supported ? 'decode-error' : 'unsupported' },
    )

  return host(
    phase,
    <video
      ref={videoRef}
      className="tg-sticker__video"
      src={src}
      poster={poster}
      muted
      loop={loop}
      playsInline
      preload="auto"
      disablePictureInPicture
      disableRemotePlayback
      aria-hidden="true"
      onLoadedData={() => setPhase((current) => (current === 'loading' ? 'static' : current))}
      onPlaying={() => setPhase('live')}
      onPause={(event) => setPhase(event.currentTarget.readyState >= 2 ? 'static' : 'loading')}
      onEnded={() => viewRef.current?.ended()}
      onError={() => {
        setFailed(src)
        errorRef.current?.(new Error('webm sticker failed to decode'))
      }}
    />,
  )
}
