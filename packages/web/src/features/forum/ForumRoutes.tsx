/**
 * The two route elements of TG-204, kept tiny and eager so the router edit is two lines;
 * the forum views themselves are lazy chunks.
 *
 *   /chat/:chatId                 → topic list when the chat is a forum, else `ChatPane`
 *   /chat/:chatId/topic/:topicId  → the topic's message view
 */
import { lazy, Suspense } from 'react'
import { Navigate, useParams } from 'react-router-dom'
import { chatListStore, selectChatById } from '@tg/core'
import { Spinner } from '@tg/ui'
import { useStore } from 'zustand/react'
import { ChatPane } from '../chat/ChatPane'

const TopicListView = lazy(() => import('./TopicListView'))
const TopicChatView = lazy(() => import('./TopicChatView'))

function Loading() {
  return (
    <div className="tg-chat">
      <Spinner label="正在加载" />
    </div>
  )
}

function useIsForum(chatId: string): boolean | undefined {
  return useStore(chatListStore, (state) => selectChatById(chatId)(state)?.is_forum)
}

export function ForumChatRoute() {
  const { chatId = '' } = useParams()
  if (useIsForum(chatId) !== true) return <ChatPane />
  return (
    <Suspense fallback={<Loading />}>
      <TopicListView key={chatId} />
    </Suspense>
  )
}

export function ForumTopicRoute() {
  const { chatId = '', topicId = '' } = useParams()
  // A chat known not to be a forum (e.g. topics just disabled) falls back to the chat.
  if (useIsForum(chatId) === false) return <Navigate to={`/chat/${encodeURIComponent(chatId)}`} replace />
  return (
    <Suspense fallback={<Loading />}>
      <TopicChatView key={`${chatId}:${topicId}`} />
    </Suspense>
  )
}
