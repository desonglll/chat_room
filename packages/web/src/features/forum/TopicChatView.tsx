/**
 * One forum topic opened (TG-204): TG-100's `ChatPane` in topic mode — the chat socket
 * filtered to this topic, history and reads on the topic endpoints — under a topic header
 * with the way back to the topic list. A closed topic replaces the composer for anyone
 * who is not a topic admin, as Telegram does.
 */
import { useCallback, useEffect, useMemo } from 'react'
import { Navigate, useNavigate, useParams } from 'react-router-dom'
import type { ForumTopic, ServerFrame, TopicReadResult, TopicsApi } from '@tg/core'
import { chatListStore, selectChatById } from '@tg/core'
import { IconButton, Spinner } from '@tg/ui'
import { useStore } from 'zustand/react'
import type { ChatPaneTopic } from '../chat/ChatPane'
import { ChatPane } from '../chat/ChatPane'
import { toggleChatInfo } from '../chat/ChatHeader'
import { ChatListIcon } from '../chatList/chatListIcons'
import { setActiveTopic } from './activeTopic'
import { TopicIcon } from './TopicIcon'
import { LockGlyph } from './TopicRow'
import { createTopicListApi, createTopicSessionMode } from './topicSessionMode'
import { topicsApi } from './topicsApi'
import { useTopicList } from './useTopicList'
import { t } from '../../i18n/index'
import './forum.css'

export const forumHref = (chatId: string) => `/chat/${encodeURIComponent(chatId)}`

function TopicHeader({ chatId, topic }: { chatId: string; topic: ForumTopic }) {
  const navigate = useNavigate()
  const chatTitle = useStore(chatListStore, (state) => selectChatById(chatId)(state)?.title ?? '')
  return (
    <header className="tg-chat__header">
      <IconButton label={t('w.forum.42929c')} variant="plain" onClick={() => void navigate(forumHref(chatId))}>
        <ChatListIcon name="back" size={22} />
      </IconButton>
      <button
        type="button"
        className="tg-chat__identity"
        onClick={() => toggleChatInfo()}
        aria-label={t('w.forum.49a948')}
      >
        <TopicIcon
          title={topic.title}
          emoji={topic.icon_emoji}
          color={topic.icon_color}
          general={topic.is_general}
          size={36}
        />
        <span className="tg-chat__titles">
          <span className="tg-chat__title">
            {topic.title}
            {topic.is_closed ? (
              <span className="tg-forum__title-lock" role="img" aria-label={t('w.forum.f62876')}>
                <LockGlyph />
              </span>
            ) : null}
          </span>
          <span className="tg-chat__subtitle">{chatTitle}</span>
        </span>
      </button>
    </header>
  )
}

export function TopicChatView({ api = topicsApi }: { api?: TopicsApi | undefined }) {
  const { chatId = '', topicId = '' } = useParams()
  const { list, error, scheduleReload } = useTopicList(api, chatId)
  const topic = list?.topics.find((row) => row.id === topicId) ?? null
  const isGeneral = topic?.is_general ?? false
  const known = topic !== null

  useEffect(
    () => (known ? setActiveTopic(chatId, isGeneral ? null : topicId) : undefined),
    [chatId, topicId, isGeneral, known],
  )

  const onRead = useCallback(
    (result: TopicReadResult) => {
      if (result.chat_read_advanced) chatListStore.getState().setUnreadCount(chatId, 0)
    },
    [chatId],
  )
  // Built once per topic: a new mode object restarts the chat session.
  const mode = useMemo(
    () => (known ? createTopicSessionMode(api, chatId, { id: topicId, is_general: isGeneral }, onRead) : null),
    [api, chatId, topicId, isGeneral, known, onRead],
  )
  const listApi = useMemo(() => createTopicListApi(api, chatId, topicId), [api, chatId, topicId])
  const onFrame = useCallback(
    (frame: ServerFrame) => {
      if (frame.type === 'topic_updated' && frame.topic.id === topicId) scheduleReload()
    },
    [topicId, scheduleReload],
  )

  if (list && !topic) return <Navigate to={forumHref(chatId)} replace />
  if (!topic || !mode) {
    return (
      <div className="tg-chat tg-forum">
        {error ? (
          <p className="tg-forum__error" role="alert">
            {error}
          </p>
        ) : (
          <div className="tg-forum__loading">
            <Spinner label={t('w.forum.22dde1')} />
          </div>
        )}
      </div>
    )
  }

  const pane: ChatPaneTopic = {
    id: topicId,
    mode,
    listApi,
    onFrame,
    header: <TopicHeader chatId={chatId} topic={topic} />,
    composerLock:
      topic.is_closed && !list?.can_manage ? (
        <div className="tg-forum__closed" role="status">
          {t('w.forum.8e3f9e')}
        </div>
      ) : null,
  }
  return <ChatPane topic={pane} />
}

export default TopicChatView
