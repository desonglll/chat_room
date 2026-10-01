/**
 * TG-408: the card under a message's text (and above the composer while typing): site, title,
 * description and an optional image. The image is loaded by the browser with no referrer;
 * the link opens in a new tab without opener access.
 */
import type { LinkPreview } from '@tg/core'

export function LinkPreviewCard({
  preview,
  onDismiss,
}: {
  preview: LinkPreview
  onDismiss?: (() => void) | undefined
}) {
  return (
    <div className="tg-link-card">
      <a className="tg-link-card__body" href={preview.url} target="_blank" rel="noopener noreferrer">
        {preview.site_name ? <span className="tg-link-card__site">{preview.site_name}</span> : null}
        {preview.title ? <span className="tg-link-card__title">{preview.title}</span> : null}
        {preview.description ? <span className="tg-link-card__description">{preview.description}</span> : null}
        {preview.image_url ? (
          <img
            className="tg-link-card__image"
            src={preview.image_url}
            alt=""
            loading="lazy"
            referrerPolicy="no-referrer"
            decoding="async"
          />
        ) : null}
      </a>
      {onDismiss ? (
        <button type="button" className="tg-link-card__dismiss" aria-label="不显示链接预览" onClick={onDismiss}>
          ×
        </button>
      ) : null}
    </div>
  )
}
