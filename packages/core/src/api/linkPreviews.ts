/**
 * TG-408 link previews: the composer asks for a link's card while typing; a sender can hide the
 * card on a sent message. Cards on messages arrive with the message or in a
 * `link_preview_updated` frame — the server builds them after delivery.
 */
import type { LinkPreview } from '../types'
import { encodePathSegment, QueryParams, type ApiClient } from './http'

export interface LinkPreviewApi {
  /** `null` when the link gets no card (refused, not HTML, no metadata, previews off). */
  get(url: string): Promise<LinkPreview | null>
  hide(chatId: string, messageId: string): Promise<void>
}

export function createLinkPreviewApi(client: ApiClient, token: () => string | null): LinkPreviewApi {
  const auth = () => {
    const value = token()
    return value ? { token: value } : {}
  }
  return {
    get: async (url) => {
      const response = await client.request('GET', '/api/link-preview', {
        ...auth(),
        query: new QueryParams({ url }),
      })
      return response.status === 204 ? null : ((await response.json()) as LinkPreview)
    },
    hide: async (chatId, messageId) => {
      await client.request(
        'DELETE',
        `/api/chats/${encodePathSegment(chatId)}/messages/${encodePathSegment(messageId)}/link-preview`,
        auth(),
      )
    },
  }
}

/** The first http(s) link in `text`, as the server picks it (trailing punctuation dropped). */
export function firstLink(text: string): string | null {
  const match = /https?:\/\/[^\s<>"`]+/i.exec(text)
  if (!match) return null
  const link = match[0].replace(/[.,;:!?)\]}'。，）]+$/u, '')
  return link.length > 'https://'.length && link.length <= 2048 ? link : null
}
