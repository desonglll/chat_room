/**
 * The group member list as a paged source. `GET /api/chats/:id/members` returns every
 * membership row in one response today (server pagination is TG-201's work: "成员列表分页
 * （20 万成员不能一次拉）"), so this source fetches once and hands out fixed-size slices
 * behind an offset cursor. When TG-201 adds a server cursor only this file changes; the
 * pager and the list component already speak cursors.
 */
import type { ApiClient, ChatMembership } from '@tg/core'
import { listChatMembers } from '@tg/core'
import type { FetchSharedPage } from './sharedPager'

export const MEMBERS_PAGE_SIZE = 50

const ROLE_ORDER: Record<ChatMembership['role'], number> = { owner: 0, admin: 1, member: 2 }

/** Telegram order: owner, admins, then online members, then by name. */
export function sortMembers(members: readonly ChatMembership[], online: ReadonlySet<string>): ChatMembership[] {
  return members
    .filter((member) => member.status === 'active')
    .sort(
      (left, right) =>
        ROLE_ORDER[left.role] - ROLE_ORDER[right.role] ||
        Number(online.has(right.user_id)) - Number(online.has(left.user_id)) ||
        memberName(left).localeCompare(memberName(right), 'zh-CN'),
    )
}

export const memberName = (member: ChatMembership): string => member.nickname || member.username

export function createMemberSource(
  load: () => Promise<ChatMembership[]>,
  online: () => ReadonlySet<string>,
  pageSize = MEMBERS_PAGE_SIZE,
): FetchSharedPage<ChatMembership> {
  let all: Promise<ChatMembership[]> | null = null
  return async (cursor) => {
    all ??= load().then((members) => sortMembers(members, online()))
    let members: ChatMembership[]
    try {
      members = await all
    } catch (error) {
      all = null // let a retry refetch
      throw error
    }
    const start = cursor === null ? 0 : Number(cursor)
    const end = start + pageSize
    return { items: members.slice(start, end), next: end < members.length ? String(end) : null }
  }
}

export function chatMemberLoader(client: ApiClient, chatId: string, token: () => string) {
  return () => listChatMembers(client, chatId, token())
}
