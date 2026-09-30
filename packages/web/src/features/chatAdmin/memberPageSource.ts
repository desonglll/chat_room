/**
 * The server-paged roster as TG-106's member-tab source. `features/chatInfo/memberSource.ts`
 * loads every membership in one request (and only for `members.review` holders); this reads
 * `GET /api/chats/:id/members/page` page by page with the server's keyset cursor, which every
 * active member may read. Swapping it in is the one-line patch in the TG-201 devlog.
 *
 * Structurally compatible with chatInfo's `FetchSharedPage<ChatMembership>`; typed locally so
 * this feature does not import another feature's internals.
 */
import type { ChatAdminApi, ChatMembership } from '@tg/core'
import { toMembership } from './chatAdminModel'

export const MEMBER_PAGE_SIZE = 50

export function createMemberPageSource(
  api: ChatAdminApi,
  chatId: string,
  pageSize = MEMBER_PAGE_SIZE,
): (cursor: string | null) => Promise<{ items: ChatMembership[]; next: string | null }> {
  return async (cursor) => {
    const page = await api.memberPage(chatId, { cursor, limit: pageSize })
    return { items: page.items.map(toMembership), next: page.next_cursor }
  }
}
