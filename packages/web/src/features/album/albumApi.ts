/**
 * TG-403 album endpoints. `send` turns fully uploaded sessions into one album atomically;
 * `recall` recalls every item the caller sent in it.
 */
import type { FetchLike, StoredMessage } from '@tg/core'

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
  400: '相册无效（需要 2–10 个已上传完成的图片或视频）',
  403: '没有在此会话发送媒体的权限',
  409: '部分文件已发送，请重新选择',
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
  if (!response.ok) throw new AlbumError(SEND_ERRORS[response.status] ?? '发送相册失败', response.status)
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
  if (!response.ok) throw new AlbumError('删除相册失败', response.status)
}
