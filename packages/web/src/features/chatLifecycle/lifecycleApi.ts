/** TG-701: the lifecycle calls, bound to the live session (core's chat client functions). */
import type { Chat, UpdateChatRequest } from '@tg/core'
import {
  authStore,
  chatListStore,
  deleteChat,
  inviteChatMember,
  leaveChat,
  selectToken,
  updateChat,
  updateChatMember,
} from '@tg/core'
import { apiClient } from '../../app/client'

const token = () => selectToken(authStore.getState()) || ''

export async function saveProfile(chatId: string, request: UpdateChatRequest): Promise<Chat> {
  const chat = await updateChat(apiClient, chatId, token(), request)
  chatListStore.getState().applyChatUpdated(chat)
  return chat
}

export async function invite(chatId: string, username: string): Promise<void> {
  await inviteChatMember(apiClient, chatId, token(), username.trim().replace(/^@/, ''))
}

export async function leave(chatId: string): Promise<void> {
  await leaveChat(apiClient, chatId, token())
  chatListStore.getState().removeChat(chatId)
}

export async function remove(chatId: string): Promise<void> {
  await deleteChat(apiClient, chatId, token())
  chatListStore.getState().removeChat(chatId)
}

export async function removeMember(chatId: string, userId: string): Promise<void> {
  await updateChatMember(apiClient, chatId, userId, token(), 'remove')
}
