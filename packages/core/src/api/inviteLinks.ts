/**
 * Invite links HTTP client (TG-205): link management under `/api/chats/:id/invite-links`, and
 * the link holder's preview / join under `/api/invite-links/:token`.
 *
 * Wire contract frozen in `docs/devlog/TG-205.md`. Join requests that arrive through an
 * approval link are settled with the existing `PATCH /api/chats/:id/members/:user_id`
 * `approve` / `reject` action (`updateChatMember` in `./chats`).
 */
import type { ChatType } from '../types'
import { ApiError, encodePathSegment, type ApiClient } from './http'

export type InviteLinkState = 'active' | 'expired' | 'limit_reached' | 'revoked'

export interface InviteLink {
  id: string
  chat_id: string
  /** The capability. Build the shareable URL with `inviteLinkUrl`. */
  token: string
  title: string
  creator_id: string | null
  creator_name: string
  expires_at: string | null
  usage_limit: number | null
  usage_count: number
  requires_approval: boolean
  is_primary: boolean
  revoked_at: string | null
  created_at: string
  pending_count: number
  state: InviteLinkState
}

export interface InviteLinksView {
  /** Primary first, then live links newest first, then revoked links. */
  links: InviteLink[]
  can_review: boolean
  can_manage_others: boolean
}

export interface InviteLinkInput {
  title: string
  /** ISO timestamp in the future, or `null` = never. */
  expires_at: string | null
  /** 1–99 999, or `null` = unlimited. Exclusive with `requires_approval`. */
  usage_limit: number | null
  requires_approval: boolean
}

export interface InviteLinkMember {
  user_id: string
  username: string
  display_name: string
  avatar_emoji: string
  status: string
  requested_at: string | null
  joined_at: string | null
}

export interface InvitePreview {
  title: string
  description: string
  avatar_emoji: string
  chat_type: ChatType
  member_count: number
  requires_approval: boolean
  membership_status: string | null
  /** Only once the viewer is an active member. */
  chat_id: string | null
}

export interface InviteJoinResult {
  status: 'active' | 'pending'
  chat_id: string | null
}

/** Why a link was refused; `internal` covers anything unexpected. */
export type InviteRefusal = 'not_found' | 'expired' | 'limit_reached' | 'revoked' | 'banned' | 'locked' | 'internal'

/** Server limits, mirrored for the form. */
export const MAX_INVITE_TITLE_CHARS = 32
export const MAX_INVITE_USAGE_LIMIT = 99_999

/** The in-app path of a link: `/joinchat/<token>`. */
export function inviteLinkPath(token: string): string {
  return `/joinchat/${encodePathSegment(token)}`
}

/** The shareable absolute URL; the host passes its own origin (core has no `location`). */
export function inviteLinkUrl(origin: string, token: string): string {
  return `${origin.replace(/\/+$/, '')}${inviteLinkPath(token)}`
}

const REFUSALS: readonly InviteRefusal[] = ['not_found', 'expired', 'limit_reached', 'revoked', 'banned', 'locked']

/** Read the refusal reason out of a failed preview / join. */
export function inviteRefusal(error: unknown): InviteRefusal {
  if (error instanceof ApiError) {
    const reason = REFUSALS.find((candidate) => candidate === error.serverMessage)
    if (reason) return reason
    if (error.status === 404) return 'not_found'
  }
  return 'internal'
}

export interface InviteLinksApi {
  list(chatId: string): Promise<InviteLinksView>
  create(chatId: string, input: InviteLinkInput): Promise<InviteLink>
  edit(chatId: string, linkId: string, input: InviteLinkInput): Promise<InviteLink>
  revoke(chatId: string, linkId: string): Promise<InviteLink>
  remove(chatId: string, linkId: string): Promise<void>
  replacePrimary(chatId: string): Promise<InviteLink>
  members(chatId: string, linkId: string): Promise<InviteLinkMember[]>
  requests(chatId: string, linkId: string): Promise<InviteLinkMember[]>
  preview(token: string): Promise<InvitePreview>
  join(token: string): Promise<InviteJoinResult>
}

export function createInviteLinksApi(client: ApiClient, token: () => string | null): InviteLinksApi {
  const auth = () => {
    const value = token()
    return value ? { token: value } : {}
  }
  const links = (chatId: string, suffix = '') => `/api/chats/${encodePathSegment(chatId)}/invite-links${suffix}`
  const link = (chatId: string, linkId: string, suffix = '') => links(chatId, `/${encodePathSegment(linkId)}${suffix}`)
  const holder = (value: string, suffix = '') => `/api/invite-links/${encodePathSegment(value)}${suffix}`
  const body = (input: InviteLinkInput) => ({
    title: input.title,
    expires_at: input.expires_at,
    usage_limit: input.usage_limit,
    requires_approval: input.requires_approval,
  })
  return {
    list: (chatId) => client.json<InviteLinksView>('GET', links(chatId), auth()),
    create: (chatId, input) => client.json<InviteLink>('POST', links(chatId), { ...auth(), body: body(input) }),
    edit: (chatId, linkId, input) =>
      client.json<InviteLink>('PUT', link(chatId, linkId), { ...auth(), body: body(input) }),
    revoke: (chatId, linkId) => client.json<InviteLink>('POST', link(chatId, linkId, '/revoke'), auth()),
    remove: async (chatId, linkId) => {
      await client.request('DELETE', link(chatId, linkId), auth())
    },
    replacePrimary: (chatId) => client.json<InviteLink>('POST', links(chatId, '/primary'), auth()),
    members: (chatId, linkId) => client.json<InviteLinkMember[]>('GET', link(chatId, linkId, '/members'), auth()),
    requests: (chatId, linkId) => client.json<InviteLinkMember[]>('GET', link(chatId, linkId, '/requests'), auth()),
    preview: (value) => client.json<InvitePreview>('GET', holder(value), auth()),
    join: (value) => client.json<InviteJoinResult>('POST', holder(value, '/join'), auth()),
  }
}
