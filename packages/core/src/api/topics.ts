/**
 * Forum topics HTTP client (TG-204). Contract: `docs/devlog/TG-204.md`, Frozen interface.
 *
 * A forum chat has exactly one General topic (`is_general`); a message whose `topic_id` is
 * null or absent belongs to it. Per-viewer fields (`unread_count`, `muted*`, `can_edit`) are
 * the caller's. Errors are bare statuses: 403 closed / no right, 404 unknown, 409 not a forum
 * (or deleting General), 400 validation.
 */
import type { Chat, StoredMessage } from '../types'
import type { ApiClient } from './http'
import { encodePathSegment, QueryParams } from './http'

/** Telegram's six topic icon colours; the server refuses any other value. */
export const TOPIC_COLORS = [0x6fb9f0, 0xffd67e, 0xcb86db, 0x8eee98, 0xff93b2, 0xfb6f5f] as const

export const MAX_TOPIC_TITLE_CHARS = 128
export const MAX_TOPIC_EMOJI_CHARS = 16

export interface ForumTopicLastMessage {
  message_id: string
  sender: string
  content: string
  created_at: string
}

export interface ForumTopic {
  id: string
  chat_id: string
  is_general: boolean
  title: string
  /** `''` = no emoji: the client draws the coloured topic glyph. */
  icon_emoji: string
  icon_custom_emoji_id: string | null
  icon_color: number
  is_pinned: boolean
  pinned_at: string | null
  is_closed: boolean
  is_hidden: boolean
  creator_id: string | null
  created_at: string
  last_message: ForumTopicLastMessage | null
  unread_count: number
  muted: boolean
  muted_until: string | null
  can_edit: boolean
}

export interface ForumTopicList {
  chat_id: string
  is_forum: boolean
  can_create: boolean
  can_manage: boolean
  /** Server order: pinned (by `pinned_at`), General, then last activity newest first. */
  topics: ForumTopic[]
}

export interface CreateTopicInput {
  title: string
  icon_emoji?: string
  icon_custom_emoji_id?: string
  icon_color?: number
}

export interface UpdateTopicInput {
  title?: string
  icon_emoji?: string
  /** `null` clears the custom emoji. */
  icon_custom_emoji_id?: string | null
  icon_color?: number
  is_closed?: boolean
  is_pinned?: boolean
  /** General only. */
  is_hidden?: boolean
}

export interface TopicReadResult {
  topic_id: string
  unread_count: number
  /** True when this read left no unread topic, so the chat cursor moved to the newest message. */
  chat_read_advanced: boolean
}

export interface TopicNotificationSettings {
  muted: boolean
  /** RFC 3339; `null` = until unmuted. */
  muted_until: string | null
}

export interface TopicsApi {
  /** Enable or disable the forum (`chat.info`); answers the updated chat descriptor. */
  setForum(chatId: string, enabled: boolean): Promise<Chat>
  list(chatId: string): Promise<ForumTopicList>
  create(chatId: string, input: CreateTopicInput): Promise<ForumTopic>
  /** `null` when the topic is gone (404). */
  get(chatId: string, topicId: string): Promise<ForumTopic | null>
  update(chatId: string, topicId: string, patch: UpdateTopicInput): Promise<ForumTopic>
  remove(chatId: string, topicId: string): Promise<void>
  /** The newest page (or the page up to `before`, a message id), ordered exactly like `/messages`. */
  messages(chatId: string, topicId: string, page?: { before?: string; limit?: number }): Promise<StoredMessage[]>
  /** Window around one message of the topic; empty when it is gone. */
  context(chatId: string, topicId: string, messageId: string, limit?: number): Promise<StoredMessage[]>
  read(chatId: string, topicId: string, messageId: string): Promise<TopicReadResult>
  setNotifications(chatId: string, topicId: string, settings: TopicNotificationSettings): Promise<ForumTopic>
}

export function createTopicsApi(client: ApiClient, token: () => string | null): TopicsApi {
  const auth = () => {
    const value = token()
    return value ? { token: value } : {}
  }
  const chat = (chatId: string, suffix = '') => `/api/chats/${encodePathSegment(chatId)}${suffix}`
  const topic = (chatId: string, topicId: string, suffix = '') =>
    chat(chatId, `/topics/${encodePathSegment(topicId)}${suffix}`)
  return {
    setForum: (chatId, enabled) => client.json<Chat>('PUT', chat(chatId, '/forum'), { ...auth(), body: { enabled } }),
    list: (chatId) => client.json<ForumTopicList>('GET', chat(chatId, '/topics'), auth()),
    create: (chatId, input) => client.json<ForumTopic>('POST', chat(chatId, '/topics'), { ...auth(), body: input }),
    get: async (chatId, topicId) => {
      const response = await client.request('GET', topic(chatId, topicId), { ...auth(), allowStatuses: [404] })
      return response.status === 404 ? null : ((await response.json()) as ForumTopic)
    },
    update: (chatId, topicId, patch) =>
      client.json<ForumTopic>('PATCH', topic(chatId, topicId), { ...auth(), body: patch }),
    remove: async (chatId, topicId) => {
      await client.request('DELETE', topic(chatId, topicId), auth())
    },
    messages: (chatId, topicId, page = {}) => {
      const query = new QueryParams({ limit: String(page.limit ?? 50) })
      if (page.before) query.set('before', page.before)
      return client.json<StoredMessage[]>('GET', topic(chatId, topicId, '/messages'), { ...auth(), query })
    },
    context: async (chatId, topicId, messageId, limit = 60) => {
      const response = await client.request(
        'GET',
        topic(chatId, topicId, `/messages/${encodePathSegment(messageId)}/context`),
        { ...auth(), query: new QueryParams({ limit: String(limit) }), allowStatuses: [404] },
      )
      return response.status === 404 ? [] : ((await response.json()) as StoredMessage[])
    },
    read: (chatId, topicId, messageId) =>
      client.json<TopicReadResult>('POST', topic(chatId, topicId, '/read'), {
        ...auth(),
        body: { message_id: messageId },
      }),
    setNotifications: (chatId, topicId, settings) =>
      client.json<ForumTopic>('PUT', topic(chatId, topicId, '/notifications'), { ...auth(), body: settings }),
  }
}

// ── Pure helpers ────────────────────────────────────────────────────────────

export const isGeneralTopic = (topic: Pick<ForumTopic, 'is_general'>): boolean => topic.is_general

/** Whether a message (stored or broadcast) belongs to a topic: null / absent ⇒ General. */
export function messageInTopic(
  message: { topic_id?: string | null | undefined },
  topic: Pick<ForumTopic, 'id' | 'is_general'>,
): boolean {
  const topicId = message.topic_id ?? null
  if (topicId === null) return topic.is_general
  return topicId === topic.id
}

/** `0x6FB9F0` → `'#6fb9f0'` for a style value; out-of-range numbers fall back to the first colour. */
export function topicColorHex(color: number): string {
  const value = Number.isInteger(color) && color >= 0 && color <= 0xffffff ? color : TOPIC_COLORS[0]
  return `#${value.toString(16).padStart(6, '0')}`
}

/** The server's order, reapplied after a local upsert: pinned by `pinned_at`, General, then activity. */
export function sortTopics(topics: readonly ForumTopic[]): ForumTopic[] {
  const activity = (topic: ForumTopic) => topic.last_message?.created_at ?? topic.created_at
  return [...topics].sort((a, b) => {
    if (a.is_pinned !== b.is_pinned) return a.is_pinned ? -1 : 1
    if (a.is_pinned) return (a.pinned_at ?? '').localeCompare(b.pinned_at ?? '')
    if (a.is_general !== b.is_general) return a.is_general ? -1 : 1
    return activity(b).localeCompare(activity(a))
  })
}
