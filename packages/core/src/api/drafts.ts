/**
 * Cloud drafts HTTP client (TG-008).
 *
 * Wire contract frozen in `docs/devlog/TG-008.md`: `PUT /api/chats/:id/draft` is an
 * idempotent save answering the stored draft, `GET` answers the stored draft or JSON `null`.
 * The response object is byte-compatible with the `draft_updated` WebSocket frame payload
 * (`docs/devlog/TG-007.md` section 3), so one parser serves both transports.
 *
 * No platform global is touched: the host injects a fetch-shaped function and a token
 * source (architecture.md section 2). `fetch` itself satisfies `DraftFetchLike`.
 */

/** A stored draft — identical to the `draft_updated` frame payload. */
export interface ChatDraft {
  user_id: string
  text: string
  reply_to_message_id: string | null
  topic_id: string | null
  updated_at: string
}

/** The PUT body. `updated_at` is deliberately absent: the server stamps every write. */
export interface DraftWrite {
  text: string
  reply_to_message_id?: string | null
  topic_id?: string | null
}

/** The slice of a fetch response the client needs; `Response` satisfies it. */
export interface FetchLikeResponse {
  status: number
  json(): Promise<unknown>
}

/** The slice of `fetch` the client needs, injectable by any host platform. */
export type DraftFetchLike = (
  url: string,
  init: {
    method: string
    headers: Record<string, string>
    body?: string
  },
) => Promise<FetchLikeResponse>

export interface DraftsApiOptions {
  /** Prepended to every path; no trailing slash. Defaults to '' (same-origin relative). */
  baseUrl?: string
  /** Bearer token source, read per request so a refreshed session needs no new client. */
  token?: () => string | null
}

export class DraftsApiError extends Error {
  constructor(
    readonly status: number,
    readonly operation: 'get' | 'put',
  ) {
    super(`drafts ${operation} failed with status ${status}`)
    this.name = 'DraftsApiError'
  }
}

export interface DraftsApi {
  /** The reconnect read path: the stored draft, or null when none is stored. */
  get(chatId: string): Promise<ChatDraft | null>
  /** Idempotent save; an identical body changes nothing server-side. */
  put(chatId: string, draft: DraftWrite): Promise<ChatDraft>
}

export function createDraftsApi(fetchLike: DraftFetchLike, options: DraftsApiOptions = {}): DraftsApi {
  const base = options.baseUrl ?? ''
  const headers = (): Record<string, string> => {
    const token = options.token?.()
    return {
      'content-type': 'application/json',
      ...(token ? { authorization: `Bearer ${token}` } : {}),
    }
  }
  return {
    async get(chatId) {
      const response = await fetchLike(`${base}/api/chats/${encodeURIComponent(chatId)}/draft`, {
        method: 'GET',
        headers: headers(),
      })
      if (response.status !== 200) throw new DraftsApiError(response.status, 'get')
      return (await response.json()) as ChatDraft | null
    },
    async put(chatId, draft) {
      const response = await fetchLike(`${base}/api/chats/${encodeURIComponent(chatId)}/draft`, {
        method: 'PUT',
        headers: headers(),
        body: JSON.stringify(draft),
      })
      if (response.status !== 200) throw new DraftsApiError(response.status, 'put')
      return (await response.json()) as ChatDraft
    },
  }
}
