/**
 * TG-1302: which URL shows an attachment *small*. Image attachments carry a server-made
 * `thumbnail_url`; chats, albums, the info panel and the viewer's strip show that and fetch the
 * original only when the viewer opens it. Older servers and non-images have none, so the
 * original is the fallback.
 *
 * GIFs keep their original: the server's GIF thumbnail is the first frame only, and a GIF's
 * motion is its content. (The still is still served for clients that cannot animate.)
 */
import type { Attachment } from '@tg/core'

type Previewable = Pick<Attachment, 'download_url' | 'thumbnail_url' | 'mime_type'>

export function attachmentPreviewUrl(attachment: Previewable): string {
  if (attachment.mime_type.toLowerCase().startsWith('image/gif')) return attachment.download_url
  return attachment.thumbnail_url || attachment.download_url
}

/**
 * `onError` for an `<img>` showing a preview: if the thumbnail cannot be served, show the
 * original instead (once). Returns `undefined` when there is nothing to fall back to.
 */
export function fallBackToOriginal(attachment: Previewable) {
  const preview = attachmentPreviewUrl(attachment)
  if (preview === attachment.download_url) return undefined
  return (event: { currentTarget: HTMLImageElement }) => {
    const image = event.currentTarget
    if (image.dataset.fellBack) return
    image.dataset.fellBack = '1'
    image.src = attachment.download_url
  }
}
