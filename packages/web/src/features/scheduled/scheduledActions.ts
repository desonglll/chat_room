/**
 * TG-404: every scheduled-message operation, API call + store update in one place, with the
 * client, token and store injected (the dialogs use `scheduledActions()`; tests use fakes).
 */
import type { ApiClient, ScheduledMessage, StoredMessage, UpdateScheduledMessageInput } from '@tg/core'
import {
  ApiError,
  createScheduledMessage,
  deleteScheduledMessage,
  listScheduledMessages,
  sendScheduledMessageNow,
  updateScheduledMessage,
} from '@tg/core'
import { activeTopicId } from '../forum/activeTopic'
import type { ScheduledStore } from './scheduledStore'

export interface ScheduledActionsDeps {
  client: ApiClient
  token(): string
  store: ScheduledStore
}

export interface ScheduleInput {
  content: string
  replyTo: string | null
  at: Date
  silent: boolean
}

export interface ScheduledActions {
  load(chatId: string): Promise<ScheduledMessage[]>
  schedule(chatId: string, input: ScheduleInput): Promise<ScheduledMessage>
  update(chatId: string, id: string, input: UpdateScheduledMessageInput): Promise<ScheduledMessage>
  remove(chatId: string, id: string): Promise<void>
  sendNow(chatId: string, id: string): Promise<StoredMessage | null>
}

/** A 404 on a per-message call means «already delivered or deleted»: drop it locally. */
const isGone = (error: unknown): boolean => error instanceof ApiError && error.status === 404

export function createScheduledActions({ client, token, store }: ScheduledActionsDeps): ScheduledActions {
  return {
    async load(chatId) {
      const items = await listScheduledMessages(client, token(), chatId)
      store.getState().replace(chatId, items)
      return items
    },

    async schedule(chatId, input) {
      const topicId = activeTopicId(chatId)
      const created = await createScheduledMessage(client, token(), chatId, {
        content: input.content,
        scheduled_at: input.at.toISOString(),
        ...(input.replyTo ? { reply_to: input.replyTo } : {}),
        ...(input.silent ? { silent: true } : {}),
        ...(topicId ? { topic_id: topicId } : {}),
      })
      store.getState().upsert(created)
      return created
    },

    async update(chatId, id, input) {
      try {
        const updated = await updateScheduledMessage(client, token(), chatId, id, input)
        store.getState().upsert(updated)
        return updated
      } catch (error) {
        if (isGone(error)) store.getState().remove(chatId, id)
        throw error
      }
    },

    async remove(chatId, id) {
      try {
        await deleteScheduledMessage(client, token(), chatId, id)
      } catch (error) {
        if (!isGone(error)) throw error
      }
      store.getState().remove(chatId, id)
    },

    async sendNow(chatId, id) {
      try {
        const message = await sendScheduledMessageNow(client, token(), chatId, id)
        store.getState().remove(chatId, id)
        return message
      } catch (error) {
        if (!isGone(error)) throw error
        store.getState().remove(chatId, id)
        return null
      }
    },
  }
}

/** User-facing copy for a failed call. */
export function scheduledErrorText(error: unknown): string {
  if (error instanceof ApiError) {
    switch (error.status) {
      case 400:
        return '时间或内容无效'
      case 403:
        return '你没有在此聊天发送消息的权限'
      case 404:
        return '这条定时消息已发送或已删除'
      case 409:
        return '此聊天的定时消息已达上限（100 条）'
      default:
        break
    }
  }
  return '操作失败，请重试'
}
