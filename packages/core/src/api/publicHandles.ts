/**
 * TG-206 public handles: set / check a chat's public username, preview a public chat by handle
 * (no internal id for non-members), join through the handle, and search public chats.
 */
import type { Chat, ChatMembership, ChatType } from '../types'
import type { ApiClient } from './http'
import { encodePathSegment, QueryParams } from './http'
import { t } from '../i18n/t'

export interface UsernameCheck {
  username: string
  available: boolean
  /** `too_short`, `too_long`, `invalid_characters`, `must_start_with_letter`,
   *  `invalid_underscores`, `reserved`, `taken`. */
  reason?: string
}

export interface PublicChatPreview {
  username: string
  title: string
  description: string
  avatar_emoji: string
  chat_type: ChatType
  member_count: number
  is_member: boolean
  /** Only for members. */
  chat_id?: string
  requires_approval: boolean
}

export interface PublicHandlesApi {
  check(username: string): Promise<UsernameCheck>
  /** `null` makes the chat private again. */
  set(chatId: string, username: string | null): Promise<Chat>
  preview(username: string): Promise<PublicChatPreview>
  join(username: string): Promise<ChatMembership>
  search(query: string): Promise<Chat[]>
}

export function createPublicHandlesApi(client: ApiClient, token: () => string | null): PublicHandlesApi {
  const auth = () => {
    const value = token()
    return value ? { token: value } : {}
  }
  return {
    check: (username) =>
      client.json<UsernameCheck>('GET', `/api/public-usernames/${encodePathSegment(username)}/check`, auth()),
    set: (chatId, username) =>
      client.json<Chat>('PUT', `/api/chats/${encodePathSegment(chatId)}/username`, { ...auth(), body: { username } }),
    preview: (username) => client.json<PublicChatPreview>('GET', `/api/public/${encodePathSegment(username)}`, auth()),
    join: (username) => client.json<ChatMembership>('POST', `/api/public/${encodePathSegment(username)}/join`, auth()),
    search: (query) =>
      client.json<Chat[]>('GET', '/api/chats/discover', { ...auth(), query: new QueryParams({ q: query }) }),
  }
}

const REASONS: Record<string, string> = {
  get too_short() {
    return t('c.api.7bb360')
  },
  get too_long() {
    return t('c.api.cede6c')
  },
  get invalid_characters() {
    return t('c.api.1ba38d')
  },
  get must_start_with_letter() {
    return t('c.api.82aae8')
  },
  get invalid_underscores() {
    return t('c.api.523a8a')
  },
  get reserved() {
    return t('c.api.8195d8')
  },
  get taken() {
    return t('c.api.dbd8c2')
  },
  get password_protected() {
    return t('c.api.e226cd')
  },
}

/** Telegram-style copy for a refused handle. */
export function usernameReasonText(reason: string | undefined): string {
  return (reason && REASONS[reason]) || t('c.api.b79dae')
}

/** The shareable link for a public chat, relative to the app's origin. */
export function publicChatPath(username: string): string {
  return `/public/${encodePathSegment(username)}`
}
