/**
 * Round video messages HTTP client (TG-402). Contract: `docs/devlog/TG-402.md`, Frozen
 * interface. Multipart like TG-401's `sendVoiceMessage`, so it takes the host's `FetchLike`.
 */
import type { ApiClient, FetchLike, StoredMessage } from '@tg/core'
import { ApiError, encodePathSegment } from '@tg/core'

/** Telegram's cap: one minute. Mirrored from `src/attachments/video_note/model.rs`. */
export const VIDEO_NOTE_MAX_MS = 60_000

export interface SendVideoNoteInput {
  chatId: string
  token: string
  /** The recording; its container is sniffed server-side. */
  video: Blob
  /** `video_note.webm` / `video_note.mp4` — the server renames it after sniffing anyway. */
  fileName: string
  /** The recorder's own measurement, used only when the container carries no duration. */
  durationMs: number
  /** A small JPEG (≤ 16 KiB), the first frame. */
  thumbnail?: Blob | null
  replyTo?: string | null
  signal?: AbortSignal
}

async function errorCode(response: Response): Promise<string> {
  try {
    const body = (await response.json()) as { error?: unknown }
    if (typeof body.error === 'string') return body.error
  } catch {
    // Non-JSON body — fall through to the status text.
  }
  return response.statusText
}

/** Upload one recording as a round video message; answers the stored message (201). */
export async function sendVideoNote(fetchImpl: FetchLike, input: SendVideoNoteInput): Promise<StoredMessage> {
  const path = `/api/chats/${encodePathSegment(input.chatId)}/video_note`
  const form = new FormData()
  form.append('file', input.video, input.fileName)
  form.append('duration_ms', String(Math.max(1, Math.round(input.durationMs))))
  if (input.thumbnail) form.append('thumbnail', input.thumbnail, 'thumbnail.jpg')
  if (input.replyTo) form.append('reply_to', input.replyTo)
  const response = await fetchImpl(path, {
    method: 'POST',
    cache: 'no-store',
    headers: { Authorization: `Bearer ${input.token}`, Accept: 'application/json' },
    body: form,
    ...(input.signal ? { signal: input.signal } : {}),
  })
  if (!response.ok) throw new ApiError(response.status, path, await errorCode(response))
  return (await response.json()) as StoredMessage
}

/**
 * Mark a video note watched by the caller. Idempotent; the sender's own call is a no-op. The
 * first view reaches the sender (and the caller's other devices) as `voice_listened`.
 */
export async function markVideoNoteListened(client: ApiClient, token: string, messageId: string): Promise<void> {
  await client.request('POST', `/api/messages/${encodePathSegment(messageId)}/video_note/listened`, { token })
}
