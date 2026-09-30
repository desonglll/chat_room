/**
 * Voice messages HTTP client (TG-401). Contract: `docs/devlog/TG-401.md`, Frozen interface.
 *
 * Sending is multipart (the recorded file plus its waveform), which `ApiClient` does not
 * speak, so it takes the host's `FetchLike` directly — like the composer's upload client.
 * Marking a message played is plain JSON-less POST through `ApiClient`.
 */
import type { StoredMessage } from '../types'
import type { ApiClient, FetchLike } from './http'
import { ApiError, encodePathSegment } from './http'

/** Telegram's waveform: 100 samples of 5 bits. Mirrored from `src/attachments/voice/model.rs`. */
export const VOICE_WAVEFORM_SAMPLES = 100
export const VOICE_WAVEFORM_MAX = 31

/** Server error code when the private-chat peer's TG-505 rule refuses voice messages. */
export const VOICE_RESTRICTED = 'voice_messages_restricted'

export interface SendVoiceInput {
  chatId: string
  token: string
  /** The recording; its container is sniffed server-side, the type here is informative. */
  audio: Blob
  /** `voice.webm` / `voice.ogg` / `voice.m4a` — the server renames it after sniffing anyway. */
  fileName: string
  /** The recorder's own measurement, used only when the container carries no duration. */
  durationMs: number
  /** Exactly `VOICE_WAVEFORM_SAMPLES` integers in `0..VOICE_WAVEFORM_MAX`. */
  waveform: readonly number[]
  replyTo?: string | null
  /** TG-204: the forum topic to post into; absent = General. */
  topicId?: string | null
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

/** Upload one recording as a voice message; answers the stored message with `voice` (201). */
export async function sendVoiceMessage(fetchImpl: FetchLike, input: SendVoiceInput): Promise<StoredMessage> {
  const path = `/api/chats/${encodePathSegment(input.chatId)}/voice`
  const form = new FormData()
  form.append('file', input.audio, input.fileName)
  form.append('waveform', input.waveform.join(','))
  form.append('duration_ms', String(Math.max(1, Math.round(input.durationMs))))
  if (input.replyTo) form.append('reply_to', input.replyTo)
  if (input.topicId) form.append('topic_id', input.topicId)
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
 * Mark a voice message played by the caller. Idempotent; the sender's own call is a no-op.
 * The first listen reaches the sender (and the caller's other devices) as `voice_listened`.
 */
export async function markVoiceListened(client: ApiClient, token: string, messageId: string): Promise<void> {
  await client.request('POST', `/api/messages/${encodePathSegment(messageId)}/voice/listened`, { token })
}
