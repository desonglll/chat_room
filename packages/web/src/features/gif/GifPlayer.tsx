/**
 * One GIF, in a bubble or in the panel. A video GIF is a muted, looping, inline `<video>`;
 * a real `.gif` is an `<img>` while playing and a canvas still otherwise — taking the
 * `<img>` out of the DOM is the only way to stop a browser decoding an animated image.
 *
 * Playback follows `createGifPlayback`: on screen and (autoplay allowed or tapped) plays,
 * off screen pauses. With `tapToToggle`, a click toggles playback (the bubble); without it
 * the parent owns the click (the panel sends the GIF).
 */
import { useCallback, useEffect, useMemo, useRef, useState, type CSSProperties } from 'react'
import { useAutoplayAllowed } from './autoplayEnvironment'
import { gifRenderMode } from './gifModel'
import { observeGifVisibility } from './gifViewport'
import { createGifPlayback } from './playback'
import { t } from '../../i18n/index'

export interface GifPlayerProps {
  src: string
  mimeType: string
  label: string
  className?: string | undefined
  style?: CSSProperties | undefined
  tapToToggle?: boolean | undefined
  /** Reports the media's own width / height once known (layout of unknown geometry). */
  onAspect?: ((aspect: number) => void) | undefined
}

function GifStill({ src, label, onAspect }: { src: string; label: string; onAspect: (aspect: number) => void }) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null)
  useEffect(() => {
    const image = new Image()
    let live = true
    image.onload = () => {
      const canvas = canvasRef.current
      if (!live || !canvas || image.naturalWidth === 0) return
      canvas.width = image.naturalWidth
      canvas.height = image.naturalHeight
      canvas.getContext('2d')?.drawImage(image, 0, 0)
      onAspect(image.naturalWidth / image.naturalHeight)
    }
    image.src = src
    return () => {
      live = false
      image.onload = null
    }
  }, [src, onAspect])
  return <canvas ref={canvasRef} className="tg-gif__media" role="img" aria-label={label} />
}

export function GifPlayer({ src, mimeType, label, className, style, tapToToggle, onAspect }: GifPlayerProps) {
  const allowed = useAutoplayAllowed()
  const [playing, setPlaying] = useState(false)
  const playback = useMemo(() => createGifPlayback(allowed, setPlaying), [allowed])
  const [root, setRoot] = useState<HTMLElement | null>(null)
  const videoRef = useRef<HTMLVideoElement | null>(null)
  const mode = gifRenderMode(mimeType)
  const aspectRef = useRef(onAspect)
  aspectRef.current = onAspect
  const reportAspect = useCallback((aspect: number) => aspectRef.current?.(aspect), [])

  useEffect(() => {
    if (!root) return
    return observeGifVisibility(root, (visible) => playback.setVisible(visible))
  }, [root, playback])

  useEffect(() => {
    const video = videoRef.current
    playback.attach(
      video
        ? {
            // Autoplay of a muted inline video is allowed everywhere; a refusal is ignored.
            play: () => void video.play().catch(() => undefined),
            pause: () => video.pause(),
          }
        : null,
    )
    return () => playback.attach(null)
  }, [playback, mode])

  const body = (
    <>
      {mode === 'video' ? (
        <video
          ref={videoRef}
          className="tg-gif__media"
          src={src}
          aria-label={label}
          muted
          loop
          playsInline
          disablePictureInPicture
          preload="metadata"
          onLoadedMetadata={(event) => {
            const { videoWidth, videoHeight } = event.currentTarget
            if (videoWidth > 0 && videoHeight > 0) reportAspect(videoWidth / videoHeight)
          }}
        />
      ) : playing ? (
        <img className="tg-gif__media" src={src} alt={label} decoding="async" draggable={false} />
      ) : (
        <GifStill src={src} label={label} onAspect={reportAspect} />
      )}
      {playing ? null : (
        <span className="tg-gif__badge" aria-hidden="true">
          GIF
        </span>
      )}
    </>
  )
  const classes = className ? `tg-gif ${className}` : 'tg-gif'
  if (!tapToToggle) {
    return (
      <span ref={setRoot} className={classes} style={style} data-playing={playing ? '' : undefined}>
        {body}
      </span>
    )
  }
  return (
    <button
      ref={setRoot}
      type="button"
      className={classes}
      style={style}
      data-playing={playing ? '' : undefined}
      aria-label={playing ? t('w.gif.f52de6', label) : t('w.gif.832ae1', label)}
      onClick={(event) => {
        event.stopPropagation()
        playback.toggle()
      }}
    >
      {body}
    </button>
  )
}
