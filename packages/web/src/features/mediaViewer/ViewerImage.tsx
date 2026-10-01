/**
 * TG-1302: the stage's photo. The original (`item.url`) is what the stage zooms and saves; while
 * it downloads, the bubble's thumbnail (`item.previewUrl`, already in the cache) fills the same
 * box underneath, so opening a photo shows it at once — sharpening, never blank. The original
 * stays transparent until it has loaded, then the preview layer goes away.
 */
import { useState, type Ref } from 'react'
import type { MediaItem } from './mediaItem'

export function ViewerImage({ item, mediaRef }: { item: MediaItem; mediaRef?: Ref<HTMLImageElement> | undefined }) {
  const hasPreview = item.previewUrl !== item.url
  const [loadedUrl, setLoadedUrl] = useState<string | null>(null)
  const loaded = !hasPreview || loadedUrl === item.url
  return (
    <>
      {loaded ? null : (
        <img
          className="tg-mv__media tg-mv__media--preview"
          src={item.previewUrl}
          alt=""
          aria-hidden="true"
          draggable={false}
          decoding="async"
        />
      )}
      <img
        ref={mediaRef}
        className="tg-mv__media"
        data-loading={loaded ? undefined : ''}
        src={item.url}
        alt={item.fileName}
        draggable={false}
        decoding="async"
        onLoad={() => setLoadedUrl(item.url)}
        // A failed original keeps the preview visible rather than an empty stage.
        onError={() => undefined}
      />
    </>
  )
}
