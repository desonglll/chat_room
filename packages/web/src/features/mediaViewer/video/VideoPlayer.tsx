/**
 * The current slide's video, played through plyr (lazy chunk, `plyrPlayer.ts`).
 *
 * plyr rewraps its `<video>` in its own DOM, which React must never reconcile, so React owns
 * only the empty host `<div>`: the video element is created, handed to plyr and destroyed
 * imperatively. The host is sized to the video's aspect ratio (known from the thumbnail or
 * from `loadedmetadata`) so the controls sit on the video, not across the whole stage.
 */
import { useEffect, useRef, useState, type CSSProperties } from 'react'
import type { MediaItem } from '../mediaItem'
import type { Size } from '../zoomGeometry'

export function VideoPlayer({ item, natural }: { item: MediaItem; natural: Size | null }) {
  const hostRef = useRef<HTMLDivElement>(null)
  const [size, setSize] = useState<Size | null>(natural)

  useEffect(() => {
    const host = hostRef.current
    if (!host) return
    const video = document.createElement('video')
    video.src = item.url
    video.playsInline = true
    video.preload = 'auto'
    video.className = 'tg-mv__video-element'
    video.addEventListener('loadedmetadata', () => {
      if (video.videoWidth > 0) setSize({ width: video.videoWidth, height: video.videoHeight })
    })
    host.replaceChildren(video)
    let cancelled = false
    let player: { destroy(): void } | null = null
    void import('./plyrPlayer').then(({ createPlayer }) => {
      if (!cancelled) player = createPlayer(video)
    })
    return () => {
      cancelled = true
      player?.destroy()
      video.pause()
      video.removeAttribute('src')
      video.load()
      host.replaceChildren()
    }
  }, [item.url])

  const style = size ? ({ '--mv-w': size.width, '--mv-h': size.height } as CSSProperties) : undefined
  return <div ref={hostRef} className="tg-mv__video" style={style} data-mv-video="" />
}
