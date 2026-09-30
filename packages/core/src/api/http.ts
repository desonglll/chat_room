/**
 * The transport every domain API module shares. Rewritten from the old client's `api.ts`
 * (TG-011): the fetch implementation is injected — there is NO global default, because
 * `fetch` is a WHATWG global and core may not touch platform globals (architecture.md §2;
 * a browser host passes `fetch`, React Native passes its own). Errors surface as `ApiError`
 * with the status and the server's `error` body field instead of hardcoded UI copy, and
 * every path targets the canonical dialect (`/api/chats/*`, request field `title`).
 */

export type FetchLike = (path: string, init?: RequestInit) => Promise<Response>

/**
 * A pure-ECMAScript query-string builder. `URLSearchParams` is a WHATWG global that stock
 * React Native/Hermes does not implement, so core builds queries with `encodeURIComponent`
 * instead (spaces become `%20` rather than `+`; the server accepts both).
 */
export class QueryParams {
  private entries: Array<[string, string]> = []

  constructor(init?: Record<string, string>) {
    if (init) for (const [key, value] of Object.entries(init)) this.entries.push([key, value])
  }

  get(key: string): string | null {
    const found = this.entries.find(([existing]) => existing === key)
    return found ? found[1] : null
  }

  set(key: string, value: string): void {
    const at = this.entries.findIndex(([existing]) => existing === key)
    if (at >= 0) this.entries[at] = [key, value]
    else this.entries.push([key, value])
  }

  toString(): string {
    return this.entries.map(([key, value]) => `${encodeURIComponent(key)}=${encodeURIComponent(value)}`).join('&')
  }
}

export class ApiError extends Error {
  constructor(
    readonly status: number,
    readonly path: string,
    /** The server's JSON `error` field when one was readable, else the status text. */
    readonly serverMessage: string,
  ) {
    super(`${status} ${path}${serverMessage ? `: ${serverMessage}` : ''}`)
    this.name = 'ApiError'
  }
}

export interface ApiClientOptions {
  fetchImpl: FetchLike
  /** Prefix for every path, no trailing slash. Empty (same-origin) by default. */
  baseUrl?: string
}

export interface RequestOptions {
  token?: string
  /** Sent as the frozen `x-room-password` header when non-empty. */
  chatPassword?: string
  query?: QueryParams
  body?: unknown
  /** Status codes the caller handles itself (e.g. 404 → null) — not thrown. */
  allowStatuses?: number[]
}

export interface ApiClient {
  request(method: string, path: string, options?: RequestOptions): Promise<Response>
  json<T>(method: string, path: string, options?: RequestOptions): Promise<T>
}

async function readServerMessage(response: Response): Promise<string> {
  try {
    const body = (await response.clone().json()) as { error?: unknown }
    if (typeof body.error === 'string') return body.error
  } catch {
    // Non-JSON body — fall through to the status text.
  }
  return response.statusText
}

export function createApiClient(options: ApiClientOptions): ApiClient {
  const fetchImpl = options.fetchImpl
  const baseUrl = options.baseUrl ?? ''

  async function request(method: string, path: string, requestOptions: RequestOptions = {}): Promise<Response> {
    const headers: Record<string, string> = { Accept: 'application/json' }
    if (requestOptions.token) headers.Authorization = `Bearer ${requestOptions.token}`
    if (requestOptions.chatPassword) headers['x-room-password'] = requestOptions.chatPassword
    const init: RequestInit = { method, cache: 'no-store', headers }
    if (requestOptions.body !== undefined) {
      headers['Content-Type'] = 'application/json'
      init.body = JSON.stringify(requestOptions.body)
    }
    const query = requestOptions.query?.toString()
    const url = `${baseUrl}${path}${query ? `?${query}` : ''}`
    const response = await fetchImpl(url, init)
    if (!response.ok && !requestOptions.allowStatuses?.includes(response.status)) {
      throw new ApiError(response.status, path, await readServerMessage(response))
    }
    return response
  }

  async function json<T>(method: string, path: string, requestOptions: RequestOptions = {}): Promise<T> {
    const response = await request(method, path, requestOptions)
    return (await response.json()) as T
  }

  return { request, json }
}

export function encodePathSegment(value: string): string {
  return encodeURIComponent(value)
}
