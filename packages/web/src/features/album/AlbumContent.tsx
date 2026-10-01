/**
 * TG-403: the album body — Telegram's mosaic (`layoutAlbum`, @tg/core) of the album's items,
 * then the album caption (stored on the first item). Each tile opens the media viewer at
 * its own attachment; the viewer pages the chat's media in history order, and album items
 * are consecutive in history, so it walks the album in order.
 *
 * Attachments carry no dimensions on the wire, so every tile starts square and the mosaic is
 * recomputed once a tile reports its natural size. The list (TG-101) measures rows, so the
 * height change is absorbed like any image loading in; the width is fixed from the start.
 */
import { useCallback, useMemo, useState } from 'react'
import type { Attachment, BroadcastMessage } from '@tg/core'
import { layoutAlbum, type AlbumItemSize } from '@tg/core'
import { MessageText, type MessageContentProps } from '../message'
import { MediaFrame } from '../message/content/MediaFrame'
import { albumItemsOf } from './albumCollapse'
import { t } from '../../i18n/index'

export const ALBUM_MAX_WIDTH = 420
const ALBUM_OPTIONS = { maxWidth: ALBUM_MAX_WIDTH, minWidth: 100, spacing: 2 }
const SQUARE: AlbumItemSize = { width: 1, height: 1 }

function isVideo(attachment: Attachment): boolean {
  return attachment.mime_type.toLowerCase().startsWith('video/')
}

export function AlbumContent({ message, actions, metaSpacer }: MessageContentProps) {
  const items = useMemo<readonly BroadcastMessage[]>(() => albumItemsOf(message) ?? [message], [message])
  const [sizes, setSizes] = useState<Record<string, AlbumItemSize>>({})
  const measured = useCallback((id: string, width: number, height: number) => {
    if (!(width > 0 && height > 0)) return
    setSizes((current) =>
      current[id]?.width === width && current[id]?.height === height
        ? current
        : { ...current, [id]: { width, height } },
    )
  }, [])
  const withMedia = items.filter((item) => item.attachment !== null)
  const layout = useMemo(
    () =>
      layoutAlbum(
        withMedia.map((item) => sizes[item.message_id] ?? SQUARE),
        ALBUM_OPTIONS,
      ),
    [withMedia, sizes],
  )
  const caption = (items[0]?.content ?? '').trim()

  return (
    <>
      <div
        className="tg-album"
        style={{ width: `${layout.width}px`, height: `${layout.height}px` }}
        data-count={withMedia.length}
      >
        {withMedia.map((item, index) => {
          const tile = layout.tiles[index]
          const attachment = item.attachment as Attachment
          if (!tile) return null
          const video = isVideo(attachment)
          return (
            <div
              key={item.message_id}
              className="tg-album__tile"
              data-top={tile.sides.top || undefined}
              data-bottom={tile.sides.bottom || undefined}
              data-left={tile.sides.left || undefined}
              data-right={tile.sides.right || undefined}
              style={{ left: `${tile.x}px`, top: `${tile.y}px`, width: `${tile.width}px`, height: `${tile.height}px` }}
            >
              <MediaFrame
                attachment={attachment}
                actions={actions}
                label={`${video ? t('w.album.af7d6e') : t('w.album.9b1861')} ${attachment.file_name}（${index + 1}/${withMedia.length}）`}
              >
                {video ? (
                  <video
                    className="tg-album__media"
                    src={attachment.download_url}
                    preload="metadata"
                    muted
                    playsInline
                    onLoadedMetadata={(event) =>
                      measured(item.message_id, event.currentTarget.videoWidth, event.currentTarget.videoHeight)
                    }
                  />
                ) : (
                  <img
                    className="tg-album__media"
                    src={attachment.download_url}
                    alt={attachment.file_name}
                    loading="lazy"
                    decoding="async"
                    draggable={false}
                    onLoad={(event) =>
                      measured(item.message_id, event.currentTarget.naturalWidth, event.currentTarget.naturalHeight)
                    }
                  />
                )}
              </MediaFrame>
            </div>
          )
        })}
      </div>
      {caption ? <MessageText text={caption} metaSpacer={metaSpacer} className="tg-bubble__caption" /> : null}
    </>
  )
}

/** The album's caption decides where the time goes: under the text, or over the mosaic. */
export function albumHasCaption(message: BroadcastMessage): boolean {
  const items = albumItemsOf(message)
  return ((items?.[0] ?? message).content ?? '').trim().length > 0
}
