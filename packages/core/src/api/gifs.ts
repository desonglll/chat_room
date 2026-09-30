/**
 * GIF HTTP client (TG-305) over the contract frozen in `docs/devlog/TG-305.md`.
 *
 * Saved GIFs are the account's own references (capability `file_url`); recent GIFs are GIF
 * messages from chats the account reads (their `file_url` is the message attachment's own
 * capability). Sending a saved or recent GIF is by reference; uploading a new one sends the
 * raw file as the request body, which `ApiClient` (JSON only) cannot, so the upload goes
 * through the injected fetch like the composer's chunk uploads.
 */
import { encodePathSegment, ApiError, QueryParams, type ApiClient, type FetchLike } from './http'
import type { StoredMessage } from '../types'

interface GifFileFields {
  mime_type: string
  size_bytes: number
  /** From the uploaded file's header when the server read it; `null` otherwise. */
  width: number | null
  height: number | null
  duration_ms: number | null
  file_url: string
}

export interface SavedGif extends GifFileFields {
  id: string
  saved_at: string
  /** Sending or re-saving moves a GIF to the front. */
  used_at: string
}

export interface RecentGif extends GifFileFields {
  message_id: string
  room_id: string
  created_at: string
}

/** Exactly one of `saved_gif_id` / `message_id`. */
export type SendGifSource = { saved_gif_id: string } | { message_id: string }

export interface SendGifOptions {
  reply_to?: string | undefined
  client_message_id?: string | undefined
}

/** The raw file of an upload: a `Blob` in a browser, whatever the host's fetch accepts. */
export type GifUploadBody = NonNullable<RequestInit['body']>

export interface GifsApi {
  saved(): Promise<SavedGif[]>
  /** Save the GIF of a message the caller can read; answers the saved entry (now first). */
  save(messageId: string): Promise<SavedGif>
  remove(savedGifId: string): Promise<void>
  recent(limit?: number): Promise<RecentGif[]>
  send(chatId: string, source: SendGifSource, options?: SendGifOptions): Promise<StoredMessage>
  upload(chatId: string, file: GifUploadBody, options?: SendGifOptions): Promise<StoredMessage>
}

export function createGifsApi(
  client: ApiClient,
  token: () => string | null,
  fetchImpl?: FetchLike,
  baseUrl = '',
): GifsApi {
  const auth = () => {
    const value = token()
    return value ? { token: value } : {}
  }
  const messagesPath = (chatId: string) => `/api/chats/${encodePathSegment(chatId)}/gif-messages`
  const options = (input: SendGifOptions = {}) => ({
    ...(input.reply_to ? { reply_to: input.reply_to } : {}),
    ...(input.client_message_id ? { client_message_id: input.client_message_id } : {}),
  })
  return {
    saved: () => client.json<SavedGif[]>('GET', '/api/gifs/saved', auth()),
    save: (messageId) =>
      client.json<SavedGif>('POST', '/api/gifs/saved', { ...auth(), body: { message_id: messageId } }),
    remove: async (savedGifId) => {
      await client.request('DELETE', `/api/gifs/saved/${encodePathSegment(savedGifId)}`, auth())
    },
    recent: (limit) =>
      client.json<RecentGif[]>('GET', '/api/gifs/recent', {
        ...auth(),
        ...(limit === undefined ? {} : { query: new QueryParams({ limit: String(limit) }) }),
      }),
    send: (chatId, source, input) =>
      client.json<StoredMessage>('POST', messagesPath(chatId), {
        ...auth(),
        body: { ...source, ...options(input) },
      }),
    upload: async (chatId, file, input) => {
      if (!fetchImpl) throw new Error('createGifsApi: upload needs a fetch implementation')
      const query = new QueryParams(options(input)).toString()
      const path = `${messagesPath(chatId)}/upload`
      const value = token()
      const response = await fetchImpl(`${baseUrl}${path}${query ? `?${query}` : ''}`, {
        method: 'POST',
        cache: 'no-store',
        headers: {
          Accept: 'application/json',
          'Content-Type': 'application/octet-stream',
          ...(value ? { Authorization: `Bearer ${value}` } : {}),
        },
        body: file,
      })
      if (!response.ok) {
        let code = response.statusText
        try {
          const body = (await response.json()) as { error?: unknown }
          if (typeof body.error === 'string') code = body.error
        } catch {
          // Non-JSON body (e.g. 413 from the body limit) — keep the status text.
        }
        throw new ApiError(response.status, path, code)
      }
      return (await response.json()) as StoredMessage
    },
  }
}
