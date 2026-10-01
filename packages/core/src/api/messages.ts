/**
 * Message history/search/forward endpoints on the canonical dialect. Rewritten from
 * `web/src/roomMessagesApi.ts` + the forward/files half of `web/src/api.ts` (TG-011).
 */
import type { ChatFilePage, ForwardResult, StoredMessage } from '../types'
import type { ApiClient } from './http'
import { encodePathSegment, QueryParams } from './http'

const messagesPath = (chatId: string, suffix = ''): string =>
  `/api/chats/${encodePathSegment(chatId)}/messages${suffix}`

export interface ChatCredentials {
  token: string
  /** Chat password for password-protected chats; empty string otherwise. */
  password?: string
}

const authOptions = ({ token, password }: ChatCredentials) => ({
  token,
  ...(password ? { chatPassword: password } : {}),
})

/** Newest page first; pass `before` (a `created_at` cursor) to walk older history. */
export function listChatMessages(
  client: ApiClient,
  chatId: string,
  credentials: ChatCredentials,
  before = '',
  limit = 50,
): Promise<StoredMessage[]> {
  const query = new QueryParams({ limit: String(limit) })
  if (before) query.set('before', before)
  return client.json<StoredMessage[]>('GET', messagesPath(chatId), { ...authOptions(credentials), query })
}

/** Window around one message for deep links; empty when the message is gone. */
export async function listChatMessageContext(
  client: ApiClient,
  chatId: string,
  messageId: string,
  credentials: ChatCredentials,
  limit = 60,
): Promise<StoredMessage[]> {
  const response = await client.request('GET', messagesPath(chatId, `/${encodePathSegment(messageId)}/context`), {
    ...authOptions(credentials),
    query: new QueryParams({ limit: String(limit) }),
    allowStatuses: [404],
  })
  if (response.status === 404) return []
  return (await response.json()) as StoredMessage[]
}

export function searchChatMessages(
  client: ApiClient,
  chatId: string,
  searchQuery: string,
  credentials: ChatCredentials,
  before = '',
  limit = 50,
): Promise<StoredMessage[]> {
  const query = new QueryParams({ q: searchQuery, limit: String(limit) })
  if (before) query.set('before', before)
  return client.json<StoredMessage[]>('GET', messagesPath(chatId, '/search'), { ...authOptions(credentials), query })
}

export function listChatFiles(
  client: ApiClient,
  chatId: string,
  credentials: ChatCredentials,
  /** TG-803: `media` / `document` / `voice` / `gif` are the info panel's tabs, classified server-side. */
  kind: 'all' | 'image' | 'video' | 'file' | 'media' | 'document' | 'voice' | 'gif',
  before = '',
  limit = 50,
): Promise<ChatFilePage> {
  const query = new QueryParams({ kind, limit: String(limit) })
  if (before) query.set('before', before)
  return client.json<ChatFilePage>('GET', `/api/chats/${encodePathSegment(chatId)}/files`, {
    ...authOptions(credentials),
    query,
  })
}

export function forwardMessages(
  client: ApiClient,
  token: string,
  messageIds: string[],
  targetChatIds: string[],
): Promise<ForwardResult[]> {
  return client.json<ForwardResult[]>('POST', '/api/messages/forward', {
    token,
    // `target_room_ids` is the frozen request spelling (TG-004 "Frozen non-rename 1").
    body: { message_ids: messageIds, target_room_ids: targetChatIds },
  })
}
