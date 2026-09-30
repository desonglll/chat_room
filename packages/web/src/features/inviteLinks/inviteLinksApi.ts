/**
 * The app-wide invite-links service: the core client with the live session's token, plus the
 * existing approve / reject action for requests that arrived through an approval link.
 */
import type { InviteLinksApi } from '@tg/core'
import { authStore, createChatAdminApi, createInviteLinksApi, selectToken, updateChatMember } from '@tg/core'
import { apiClient } from '../../app/client'

export interface InviteLinksService extends InviteLinksApi {
  settleRequest(chatId: string, userId: string, approve: boolean): Promise<void>
  /** The signed-in account, to tell the viewer's own links apart. */
  viewerId(): string | null
  /**
   * Whether to show the entry at all: `members.invite` held as owner or administrator (the
   * server's rule). Read from `GET /permissions`, which has no side effects — unlike the
   * list, which issues the primary link on first read.
   */
  canManage(chatId: string): Promise<boolean>
}

const token = () => selectToken(authStore.getState()) || null
const permissions = createChatAdminApi(apiClient, token)

export const inviteLinksApi: InviteLinksService = {
  ...createInviteLinksApi(apiClient, token),
  settleRequest: async (chatId, userId, approve) => {
    await updateChatMember(apiClient, chatId, userId, token() ?? '', approve ? 'approve' : 'reject')
  },
  viewerId: () => authStore.getState().session?.user.id ?? null,
  canManage: async (chatId) => {
    const view = await permissions.permissions(chatId)
    return view.my_permissions.includes('members.invite') && (view.my_role === 'owner' || view.my_role === 'admin')
  },
}
