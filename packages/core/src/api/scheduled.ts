/**
 * Scheduled messages HTTP client (TG-404). Contract: `docs/devlog/TG-404.md`, Frozen interface.
 *
 * A scheduled message is visible only to its author until it is delivered; then it becomes an
 * ordinary message **with the same id**, arriving as a normal `broadcast` frame. A client can
 * therefore drop a list entry when a broadcast with its id appears.
 */
import type { MessageEntity, StoredMessage } from '../types'
import type { ApiClient } from './http'
import { encodePathSegment } from './http'

/** Mirrored from `src/messages/scheduled/model.rs`. */
export const SCHEDULED_LIMITS = {
  perChat: 100,
  maxDaysAhead: 366,
} as const

export interface ScheduledMessage {
  id: string
  chat_id: string
  content: string
  entities?: MessageEntity[]
  reply_to: string | null
  silent: boolean
  /** ISO-8601 UTC. */
  scheduled_at: string
  created_at: string
  updated_at: string
}

export interface CreateScheduledMessageInput {
  content: string
  /** ISO-8601; must be in the future and at most a year ahead (400 otherwise). */
  scheduled_at: string
  entities?: MessageEntity[]
  reply_to?: string
  silent?: boolean
}

/** Absent fields keep their value. `entities` only applies together with `content`. */
export interface UpdateScheduledMessageInput {
  content?: string
  entities?: MessageEntity[]
  scheduled_at?: string
  silent?: boolean
}

const base = (chatId: string): string => `/api/chats/${encodePathSegment(chatId)}/scheduled-messages`
const one = (chatId: string, id: string, suffix = ''): string => `${base(chatId)}/${encodePathSegment(id)}${suffix}`

/** The caller's own scheduled messages in a chat, soonest first. */
export function listScheduledMessages(client: ApiClient, token: string, chatId: string) {
  return client.json<ScheduledMessage[]>('GET', base(chatId), { token })
}

export function createScheduledMessage(
  client: ApiClient,
  token: string,
  chatId: string,
  input: CreateScheduledMessageInput,
) {
  return client.json<ScheduledMessage>('POST', base(chatId), { token, body: input })
}

export function updateScheduledMessage(
  client: ApiClient,
  token: string,
  chatId: string,
  id: string,
  input: UpdateScheduledMessageInput,
) {
  return client.json<ScheduledMessage>('PATCH', one(chatId, id), { token, body: input })
}

/** Cancel (HTTP 204). A 404 means it was already delivered or deleted. */
export async function deleteScheduledMessage(client: ApiClient, token: string, chatId: string, id: string) {
  await client.request('DELETE', one(chatId, id), { token })
}

/** Deliver now; answers the delivered message (its id equals the scheduled id). */
export function sendScheduledMessageNow(client: ApiClient, token: string, chatId: string, id: string) {
  return client.json<StoredMessage>('POST', one(chatId, id, '/send-now'), { token })
}
