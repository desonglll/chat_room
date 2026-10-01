/**
 * TG-802: a live timeline message → the same media kind the server's chat-list preview
 * carries (`src/conversations/preview_media.rs`), so a preview mirrored from an open chat
 * and one fetched from `/api/conversations` never disagree. Pure.
 */
import type { MessageLocation, PollState, PreviewMediaKind } from '@tg/core'

export interface PreviewMediaSource {
  media_kind?: string | undefined
  poll?: PollState | undefined
  location?: MessageLocation | undefined
  grouped_id?: string | undefined
  attachment?: { mime_type: string } | null | undefined
}

const DEDICATED = new Set(['voice', 'video_note', 'sticker', 'gif', 'contact'])

export function previewMediaKind(message: PreviewMediaSource): PreviewMediaKind | null {
  if (message.poll) return 'poll'
  const kind = message.media_kind
  if (kind && DEDICATED.has(kind)) return kind as PreviewMediaKind
  if (kind === 'location' || message.location) return message.location?.live_until ? 'live_location' : 'location'
  const mime = message.attachment?.mime_type
  if (!mime) return null
  const family = mime.split('/')[0]
  const byMime: PreviewMediaKind =
    mime === 'image/gif'
      ? 'gif'
      : family === 'image'
        ? 'photo'
        : family === 'video'
          ? 'video'
          : family === 'audio'
            ? 'audio'
            : 'file'
  return message.grouped_id && (byMime === 'photo' || byMime === 'video') ? 'album' : byMime
}
