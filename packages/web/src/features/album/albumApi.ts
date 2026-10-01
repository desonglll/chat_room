/**
 * TG-403 album endpoints. `send` turns fully uploaded sessions into one album atomically;
 * `recall` recalls every item the caller sent in it.
 */
import type { FetchLike, StoredMessage } from '@tg/core'
import { t } from '../../i18n/index'

export interface SendAlbumInput {
  chatId: string
  token: string
  uploadIds: string[]
  caption: string
  replyTo: string | null
  /** TG-204: the forum topic to post into; null = General. */
  topicId: string | null
}

export class AlbumError extends Error {
  constructor(
    message: string,
    readonly status: number,
  ) {
    super(message)
    this.name = 'AlbumError'
  }
}

const SEND_ERRORS: Record<number, string> = {
  get 400() {
    return t('w.album.51412d')
  },
  get 403() {
    return t('w.album.1655e4')
  },
  get 409() {
    return t('w.album.8346a8')
  },
}

export async function sendAlbum(
  fetchImpl: FetchLike,
  input: SendAlbumInput,
): Promise<{ grouped_id: string; messages: StoredMessage[] }> {
  const response = await fetchImpl(`/api/chats/${encodeURIComponent(input.chatId)}/albums`, {
    method: 'POST',
    cache: 'no-store',
    headers: { Authorization: `Bearer ${input.token}`, 'Content-Type': 'application/json' },
    body: JSON.stringify({
      upload_ids: input.uploadIds,
      caption: input.caption,
      reply_to: input.replyTo,
      ...(input.topicId ? { topic_id: input.topicId } : {}),
    }),
  })
  if (!response.ok) throw new AlbumError(SEND_ERRORS[response.status] ?? t('w.album.2b5b43'), response.status)
  return (await response.json()) as { grouped_id: string; messages: StoredMessage[] }
}

export async function recallAlbum(
  fetchImpl: FetchLike,
  input: { chatId: string; token: string; groupedId: string },
): Promise<void> {
  const response = await fetchImpl(
    `/api/chats/${encodeURIComponent(input.chatId)}/albums/${encodeURIComponent(input.groupedId)}`,
    { method: 'DELETE', cache: 'no-store', headers: { Authorization: `Bearer ${input.token}` } },
  )
  if (!response.ok) throw new AlbumError(t('w.album.988da6'), response.status)
}
