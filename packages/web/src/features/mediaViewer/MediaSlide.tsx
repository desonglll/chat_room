/**
 * One slide of the stage track. The active slide gets the zoomable `<img>` (its ref feeds the
 * gesture hook) or the plyr player; neighbours render a still (image, or a video's first
 * frame) so a swipe reveals real content. A sensitive medium stays veiled until revealed,
 * like the bubble's veil.
 */
import type { Ref } from 'react'
import type { MediaItem } from './mediaItem'
import type { Size } from './zoomGeometry'
import { VideoPlayer } from './video/VideoPlayer'
import { PlayGlyph } from './icons'
import { t } from '../../i18n/index'

export interface MediaSlideProps {
  item: MediaItem
  active: boolean
  revealed: boolean
  onReveal(): void
  mediaRef?: Ref<HTMLImageElement> | undefined
  /** Natural size known before load (from the thumbnail), for the video host. */
  natural?: Size | null | undefined
}

export function MediaSlide({ item, active, revealed, onReveal, mediaRef, natural = null }: MediaSlideProps) {
  if (item.isSensitive && !revealed) {
    return (
      <button type="button" className="tg-mv__veil" onClick={onReveal} data-mv-chrome="">
        {item.kind === 'image' ? (
          <img className="tg-mv__media tg-mv__media--veiled" src={item.url} alt="" draggable={false} />
        ) : null}
        <span className="tg-mv__veil-label">{t('w.mediaViewer.8c0540')}</span>
      </button>
    )
  }
  if (item.kind === 'image') {
    return (
      <img
        ref={active ? mediaRef : undefined}
        className="tg-mv__media"
        src={item.url}
        alt={item.fileName}
        draggable={false}
        decoding="async"
      />
    )
  }
  if (active) return <VideoPlayer key={item.url} item={item} natural={natural} />
  return (
    <span className="tg-mv__still">
      <video className="tg-mv__media" src={item.url} preload="metadata" muted playsInline />
      <span className="tg-mv__still-play">
        <PlayGlyph />
      </span>
    </span>
  )
}
