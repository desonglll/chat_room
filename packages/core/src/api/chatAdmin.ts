/**
 * Chat administration HTTP client (TG-201): permissions, group defaults, the keyset-paged
 * roster, administrator appointment and member restrictions.
 *
 * Wire contract frozen in `docs/devlog/TG-201.md`. Every endpoint re-authorizes server-side on
 * each request; what the client receives already reflects the viewer's current rights (an
 * entry's `admin_rights` / `restrictions` are omitted when the viewer may not see them).
 */
import type { ChatType } from '../types'
import { encodePathSegment, QueryParams, type ApiClient } from './http'

export type PermissionScope = 'member' | 'admin' | 'owner'

export interface PermissionDescriptor {
  key: string
  scope: PermissionScope
  /** Chinese label, in display order. */
  label: string
}

export interface ChatPermissionsView {
  chat_id: string
  chat_type: ChatType
  member_count: number
  /** Member-scope keys every ordinary member holds. */
  default_permissions: string[]
  /** Every key the viewer holds now, restrictions and chat type applied. */
  my_permissions: string[]
  my_role: 'owner' | 'admin' | 'member' | ''
  registry: PermissionDescriptor[]
}

export interface MemberRestriction {
  permission_key: string
  /** ISO timestamp; `null` = until lifted. */
  until: string | null
}

export interface ChatMemberEntry {
  user_id: string
  username: string
  display_name: string
  avatar_emoji: string
  nickname: string
  role: 'owner' | 'admin' | 'member'
  status: string
  joined_at: string | null
  custom_title: string
  admin_rights?: string[]
  restrictions?: MemberRestriction[]
}

export interface ChatMemberPage {
  items: ChatMemberEntry[]
  next_cursor: string | null
}

export type MemberFilter = 'all' | 'admins' | 'restricted'

/** The member-toggleable keys (group defaults and restrictions), in server order. */
export const MEMBER_TOGGLEABLE_KEYS = [
  'message.send',
  'message.send_media',
  'message.send_sticker',
  'message.send_poll',
  'message.embed_link',
  'members.invite',
  'message.pin',
  'chat.info',
  'chat.topics',
] as const

/** The checkboxes of the administrator form, in server order. */
export const ADMIN_ASSIGNABLE_KEYS = [
  'chat.info',
  'room.settings',
  'message.post',
  'message.edit_any',
  'message.delete_any',
  'message.pin',
  'members.review',
  'members.invite',
  'members.remove',
  'members.ban',
  'members.promote',
  'chat.topics',
  'chat.anonymous',
  'chat.call',
] as const

/** Server limit for an administrator title. */
export const MAX_ADMIN_TITLE_CHARS = 16

export interface ChatAdminApi {
  permissions(chatId: string): Promise<ChatPermissionsView>
  setDefaultPermissions(chatId: string, permissions: readonly string[]): Promise<ChatPermissionsView>
  memberPage(
    chatId: string,
    options?: { cursor?: string | null; limit?: number; filter?: MemberFilter },
  ): Promise<ChatMemberPage>
  member(chatId: string, userId: string): Promise<ChatMemberEntry>
  appointAdmin(
    chatId: string,
    userId: string,
    permissions: readonly string[],
    customTitle: string,
  ): Promise<ChatMemberEntry>
  dismissAdmin(chatId: string, userId: string): Promise<ChatMemberEntry>
  restrictMember(
    chatId: string,
    userId: string,
    denied: readonly string[],
    until: string | null,
  ): Promise<ChatMemberEntry>
}

export function createChatAdminApi(client: ApiClient, token: () => string | null): ChatAdminApi {
  const auth = () => {
    const value = token()
    return value ? { token: value } : {}
  }
  const chat = (chatId: string, suffix = '') => `/api/chats/${encodePathSegment(chatId)}${suffix}`
  const member = (chatId: string, userId: string, suffix = '') =>
    chat(chatId, `/members/${encodePathSegment(userId)}${suffix}`)
  return {
    permissions: (chatId) => client.json<ChatPermissionsView>('GET', chat(chatId, '/permissions'), auth()),
    setDefaultPermissions: (chatId, permissions) =>
      client.json<ChatPermissionsView>('PUT', chat(chatId, '/default-permissions'), {
        ...auth(),
        body: { permissions: [...permissions] },
      }),
    memberPage: (chatId, options = {}) => {
      const params: Record<string, string> = {}
      if (options.cursor) params.cursor = options.cursor
      if (options.limit !== undefined) params.limit = String(options.limit)
      if (options.filter && options.filter !== 'all') params.filter = options.filter
      return client.json<ChatMemberPage>('GET', chat(chatId, '/members/page'), {
        ...auth(),
        query: new QueryParams(params),
      })
    },
    member: (chatId, userId) => client.json<ChatMemberEntry>('GET', member(chatId, userId), auth()),
    appointAdmin: (chatId, userId, permissions, customTitle) =>
      client.json<ChatMemberEntry>('PUT', member(chatId, userId, '/admin'), {
        ...auth(),
        body: { permissions: [...permissions], custom_title: customTitle },
      }),
    dismissAdmin: (chatId, userId) => client.json<ChatMemberEntry>('DELETE', member(chatId, userId, '/admin'), auth()),
    restrictMember: (chatId, userId, denied, until) =>
      client.json<ChatMemberEntry>('PUT', member(chatId, userId, '/restrictions'), {
        ...auth(),
        body: { denied_permissions: [...denied], until },
      }),
  }
}
