/**
 * Channel HTTP client (TG-202): creation, silent subscriptions, the signature switch and the
 * view report. Wire contract frozen in `docs/devlog/TG-202.md`. Canonical `/api/chats` only.
 *
 * Posting itself is not here: a channel post is an ordinary message send, which the server
 * accepts only from holders of `message.post`.
 */
import type { Chat, ChatMembership, MessageViewCount } from '../types'
import { encodePathSegment, type ApiClient } from './http'

/** Posts one view report may carry (server `MAX_VIEWED_POSTS`). */
export const MAX_VIEWED_POSTS = 100

export interface CreateChannelInput {
  title: string
  description?: string
  signaturesEnabled?: boolean
}

export type SubscriptionState = 'active' | 'pending'

export interface ChannelApi {
  create(input: CreateChannelInput): Promise<Chat>
  /** `'pending'` when the channel approves subscribers. */
  subscribe(chatId: string, password?: string): Promise<{ state: SubscriptionState; membership: ChatMembership }>
  unsubscribe(chatId: string): Promise<void>
  setSignatures(chatId: string, enabled: boolean): Promise<Chat>
  /** Report posts on screen (≤ {@link MAX_VIEWED_POSTS}); answers the counts as stored now. */
  reportViews(chatId: string, messageIds: readonly string[]): Promise<MessageViewCount[]>
}

export function createChannelApi(client: ApiClient, token: () => string | null): ChannelApi {
  const auth = () => {
    const value = token()
    return value ? { token: value } : {}
  }
  const chat = (chatId: string, suffix: string) => `/api/chats/${encodePathSegment(chatId)}${suffix}`
  return {
    create: (input) =>
      client.json<Chat>('POST', '/api/chats', {
        ...auth(),
        body: {
          title: input.title.trim(),
          password: null,
          join_policy: 'open',
          chat_type: 'channel',
          signatures_enabled: input.signaturesEnabled ?? false,
          ...(input.description?.trim() ? { description: input.description.trim() } : {}),
        },
      }),
    subscribe: async (chatId, password) => {
      const response = await client.request('POST', chat(chatId, '/subscription'), {
        ...auth(),
        ...(password ? { body: { password } } : {}),
      })
      const membership = (await response.json()) as ChatMembership
      return { state: response.status === 202 ? 'pending' : 'active', membership }
    },
    unsubscribe: async (chatId) => {
      await client.request('DELETE', chat(chatId, '/subscription'), auth())
    },
    setSignatures: (chatId, enabled) =>
      client.json<Chat>('PATCH', chat(chatId, '/channel'), { ...auth(), body: { signatures_enabled: enabled } }),
    reportViews: async (chatId, messageIds) => {
      const answer = await client.json<{ views: MessageViewCount[] }>('POST', chat(chatId, '/message-views'), {
        ...auth(),
        body: { message_ids: messageIds.slice(0, MAX_VIEWED_POSTS) },
      })
      return answer.views
    },
  }
}
