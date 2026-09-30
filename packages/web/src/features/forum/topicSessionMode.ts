/**
 * Binds one forum topic to the chat session and the virtual list (TG-204): which live
 * broadcasts belong to it (null `topic_id` ⇒ General), where its history and catch-up
 * pages come from, and where its reads go. Framework-free, so the session tests drive it.
 */
import type { ForumTopic, TopicReadResult, TopicsApi } from '@tg/core'
import { messageInTopic, storedMessageToBroadcast } from '@tg/core'
import type { ChatSessionTopicMode } from '../chat/chatSession'
import type { MessageListApi } from '../messageList/messageListController'

export type TopicRef = Pick<ForumTopic, 'id' | 'is_general'>

export function createTopicSessionMode(
  api: TopicsApi,
  chatId: string,
  topic: TopicRef,
  onRead?: (result: TopicReadResult) => void,
): ChatSessionTopicMode {
  return {
    sendTopicId: topic.is_general ? null : topic.id,
    accepts: (frame) => messageInTopic(frame, topic),
    latest: () => api.messages(chatId, topic.id),
    read: (messageId) => {
      api.read(chatId, topic.id, messageId).then(onRead, () => {
        // The next arrival or visibility change retries with a newer message.
      })
    },
  }
}

/** The list's older pages and jump windows, from the topic endpoints. */
export function createTopicListApi(api: TopicsApi, chatId: string, topicId: string): MessageListApi {
  return {
    loadOlder: async (beforeId, limit) =>
      (await api.messages(chatId, topicId, { before: beforeId, limit })).map(storedMessageToBroadcast),
    loadAround: async (messageId, limit) =>
      (await api.context(chatId, topicId, messageId, limit)).map(storedMessageToBroadcast),
  }
}
