/**
 * Chat lifecycle + membership endpoints, canonical dialect only (`/api/chats/*`,
 * TG-004 devlog §5). Rewritten from the chat half of `web/src/api.ts` (TG-011): requests
 * send `title` (never the alias-only `name`), responses are the canonical `Chat`
 * descriptor. The deprecated `/api/rooms/*` alias is for the frozen clients, not for us.
 */
import type { Chat, ChatMembership, CreateChatRequest, JoinPolicy, MembershipRole, UpdateChatRequest } from '../types'
import type { ApiClient } from './http'
import { encodePathSegment } from './http'

const chatPath = (chatId: string, suffix = ''): string => `/api/chats/${encodePathSegment(chatId)}${suffix}`

export function listChats(client: ApiClient, token = ''): Promise<Chat[]> {
  return client.json<Chat[]>('GET', '/api/chats', token ? { token } : {})
}

export function discoverChats(client: ApiClient, token: string): Promise<Chat[]> {
  return client.json<Chat[]>('GET', '/api/chats/discover', { token })
}

export async function getChat(client: ApiClient, chatId: string, token = ''): Promise<Chat | null> {
  const response = await client.request('GET', chatPath(chatId), { token, allowStatuses: [404] })
  if (response.status === 404) return null
  return (await response.json()) as Chat
}

export function createChat(client: ApiClient, token: string, request: CreateChatRequest): Promise<Chat> {
  return client.json<Chat>('POST', '/api/chats', { token, body: request })
}

export function updateChat(
  client: ApiClient,
  chatId: string,
  token: string,
  request: UpdateChatRequest,
): Promise<Chat> {
  return client.json<Chat>('PATCH', chatPath(chatId), { token, body: request })
}

export async function deleteChat(client: ApiClient, chatId: string, token: string): Promise<void> {
  await client.request('DELETE', chatPath(chatId), { token })
}

export function requestChatJoin(
  client: ApiClient,
  chatId: string,
  token: string,
  password: string,
): Promise<ChatMembership> {
  return client.json<ChatMembership>('POST', chatPath(chatId, '/join-requests'), {
    token,
    body: { password: password || null },
  })
}

export async function leaveChat(client: ApiClient, chatId: string, token: string): Promise<void> {
  await client.request('DELETE', chatPath(chatId, '/members/me'), { token })
}

export function listChatMembers(client: ApiClient, chatId: string, token: string): Promise<ChatMembership[]> {
  return client.json<ChatMembership[]>('GET', chatPath(chatId, '/members'), { token })
}

export function inviteChatMember(
  client: ApiClient,
  chatId: string,
  token: string,
  username: string,
): Promise<ChatMembership> {
  return client.json<ChatMembership>('POST', chatPath(chatId, '/invitations'), { token, body: { username } })
}

export function setChatNickname(
  client: ApiClient,
  chatId: string,
  token: string,
  nickname: string,
): Promise<ChatMembership> {
  return client.json<ChatMembership>('PATCH', chatPath(chatId, '/members/me'), { token, body: { nickname } })
}

export type MemberAction = 'approve' | 'reject' | 'remove' | 'set_role' | 'ban' | 'unban'

export function updateChatMember(
  client: ApiClient,
  chatId: string,
  userId: string,
  token: string,
  action: MemberAction,
  role?: Exclude<MembershipRole, 'owner'>,
): Promise<ChatMembership> {
  return client.json<ChatMembership>('PATCH', chatPath(chatId, `/members/${encodePathSegment(userId)}`), {
    token,
    body: { action, role },
  })
}

export type { JoinPolicy }
