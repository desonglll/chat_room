/**
 * Polls and quizzes HTTP client (TG-406). Contract: `docs/devlog/TG-406.md`, Frozen interface.
 *
 * A poll is keyed by its message: `message_id` below is the poll message's id and equals
 * `PollState.id`. Every call answers the poll *as the caller sees it* (own `chosen`, quiz
 * answer once answered); live chat-wide changes arrive as `poll_updated` frames.
 */
import type { PollState, StoredMessage } from '../types'
import type { ApiClient } from './http'
import { encodePathSegment, QueryParams } from './http'

/** Telegram's limits, mirrored from `src/messages/polls/model.rs`. */
export const POLL_LIMITS = {
  questionChars: 300,
  minOptions: 2,
  maxOptions: 10,
  optionChars: 100,
  explanationChars: 200,
} as const

export interface CreatePollInput {
  question: string
  options: string[]
  /** Default false: anonymous, as in Telegram. */
  public_voters?: boolean
  multiple_choice?: boolean
  quiz?: boolean
  /** Required for a quiz, refused otherwise. */
  correct_option?: number
  /** Quiz only. */
  explanation?: string
  reply_to?: string
  /** Idempotency key; a retry with the same key answers the first message. */
  client_message_id?: string
  /** TG-204: the forum topic; omitted for General. */
  topic_id?: string
}

export interface PollVoter {
  user_id: string
  username: string
  display_name: string
  avatar_emoji: string
  voted_at: string
}

export interface PollVoterPage {
  option: number
  total: number
  voters: PollVoter[]
}

const pollPath = (messageId: string, suffix = ''): string => `/api/polls/${encodePathSegment(messageId)}${suffix}`

/** Create a poll message; answers the stored message with its `poll` (HTTP 201). */
export function createPoll(client: ApiClient, token: string, chatId: string, input: CreatePollInput) {
  return client.json<StoredMessage>('POST', `/api/chats/${encodePathSegment(chatId)}/polls`, { token, body: input })
}

export function getPoll(client: ApiClient, token: string, messageId: string) {
  return client.json<PollState>('GET', pollPath(messageId), { token })
}

/** Cast or replace the caller's ballot (option indexes; exactly one unless multiple-choice). */
export function votePoll(client: ApiClient, token: string, messageId: string, options: number[]) {
  return client.json<PollState>('POST', pollPath(messageId, '/votes'), { token, body: { options } })
}

/** Retract the caller's ballot. A quiz answers 409: quiz answers are final. */
export function retractPollVote(client: ApiClient, token: string, messageId: string) {
  return client.json<PollState>('DELETE', pollPath(messageId, '/votes'), { token })
}

/** Stop the poll: its author or a chat administrator only (403 otherwise). */
export function closePoll(client: ApiClient, token: string, messageId: string) {
  return client.json<PollState>('POST', pollPath(messageId, '/close'), { token })
}

/** One page of an option's voters, newest first. Anonymous polls answer 403. */
export function listPollVoters(
  client: ApiClient,
  token: string,
  messageId: string,
  option: number,
  page: { limit?: number; offset?: number } = {},
) {
  const query = new QueryParams({ option: String(option) })
  if (page.limit !== undefined) query.set('limit', String(page.limit))
  if (page.offset !== undefined) query.set('offset', String(page.offset))
  return client.json<PollVoterPage>('GET', pollPath(messageId, '/voters'), { token, query })
}
