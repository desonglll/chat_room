/**
 * TG-702 contacts: friends, friend requests, remarks, the blocklist, user search and opening a
 * private chat (`src/social`). The server applies every rule (blocks stop requests and chats).
 */
import { encodePathSegment, QueryParams, type ApiClient } from './http'
import type { UserStatusEntry } from '../types'

export interface SocialUser {
  id: string
  username: string
  avatar_emoji: string
  display_name: string
  signature: string
  remark: string
  /** `friend`, `blocked`, `incoming` / `outgoing` (a pending request) or `none`. */
  relationship: string
}

export interface FriendRequestView {
  user: { id: string; username: string; avatar_emoji: string; display_name: string }
  direction: 'incoming' | 'outgoing'
  created_at: string
}

export interface SocialApi {
  friends(): Promise<SocialUser[]>
  /** TG-903: each friend's online / last-seen status, already filtered by privacy server-side. */
  friendStatuses(): Promise<UserStatusEntry[]>
  requests(direction: 'incoming' | 'outgoing'): Promise<FriendRequestView[]>
  sendRequest(userId: string): Promise<void>
  respond(userId: string, accept: boolean): Promise<void>
  cancelRequest(userId: string): Promise<void>
  removeFriend(userId: string): Promise<void>
  setRemark(userId: string, remark: string): Promise<void>
  blocks(): Promise<SocialUser[]>
  block(userId: string): Promise<void>
  unblock(userId: string): Promise<void>
  search(query: string): Promise<SocialUser[]>
  /** Open (or create) the private chat with `userId`; answers its chat id. */
  openPrivateChat(userId: string): Promise<string>
}

export function createSocialApi(client: ApiClient, token: () => string | null): SocialApi {
  const auth = () => {
    const value = token()
    return value ? { token: value } : {}
  }
  const id = encodePathSegment
  return {
    friends: () => client.json<SocialUser[]>('GET', '/api/friends', auth()),
    friendStatuses: () => client.json<UserStatusEntry[]>('GET', '/api/friends/statuses', auth()),
    requests: (direction) =>
      client.json<FriendRequestView[]>('GET', '/api/friend-requests', {
        ...auth(),
        query: new QueryParams({ direction }),
      }),
    sendRequest: async (userId) => {
      await client.request('POST', '/api/friend-requests', { ...auth(), body: { user_id: userId } })
    },
    respond: async (userId, accept) => {
      await client.request('PATCH', `/api/friend-requests/${id(userId)}`, {
        ...auth(),
        body: { action: accept ? 'accept' : 'decline' },
      })
    },
    cancelRequest: async (userId) => {
      await client.request('DELETE', `/api/friend-requests/${id(userId)}`, auth())
    },
    removeFriend: async (userId) => {
      await client.request('DELETE', `/api/friends/${id(userId)}`, auth())
    },
    setRemark: async (userId, remark) => {
      await client.request('PUT', `/api/friends/${id(userId)}/remark`, { ...auth(), body: { remark } })
    },
    blocks: () => client.json<SocialUser[]>('GET', '/api/blocks', auth()),
    block: async (userId) => {
      await client.request('PUT', `/api/blocks/${id(userId)}`, auth())
    },
    unblock: async (userId) => {
      await client.request('DELETE', `/api/blocks/${id(userId)}`, auth())
    },
    search: (query) =>
      client.json<SocialUser[]>('GET', '/api/users/search', {
        ...auth(),
        query: new QueryParams({ q: query, limit: '20' }),
      }),
    openPrivateChat: async (userId) =>
      (await client.json<{ room_id: string }>('POST', '/api/direct-chats', { ...auth(), body: { user_id: userId } }))
        .room_id,
  }
}

/** The name a contact is shown under: your remark, then their display name, then @username. */
export function contactName(user: Pick<SocialUser, 'remark' | 'display_name' | 'username'>): string {
  return user.remark || user.display_name || user.username
}
