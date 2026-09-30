/**
 * A forum chat opened (TG-204): Telegram's topic list in the middle pane. The chat socket
 * stays open (without the chat-level read frame: a forum reads per topic) so topic and
 * message frames refresh the list live. Rows open `/chat/:chatId/topic/:topicId`.
 */
import { useCallback, useEffect, useMemo, useState } from 'react'
import { useParams } from 'react-router-dom'
import type { ForumTopic, ServerFrame, TopicsApi } from '@tg/core'
import { chatListStore, selectChatById, uiStore } from '@tg/core'
import { Avatar, Button, Modal, ScrollArea, Spinner } from '@tg/ui'
import { useStore } from 'zustand/react'
import { CONNECTION_COPY, toggleChatInfo } from '../chat/ChatHeader'
import { useChatSession } from '../chat/useChatSession'
import { useMinuteClock } from '../chatList/useMinuteClock'
import { MobileBackButton } from '../shell/MobileBackButton'
import { TopicDialog } from './TopicDialog'
import type { TopicAction } from './topicListModel'
import { visibleTopics } from './topicListModel'
import { TopicRow } from './TopicRow'
import { topicsApi } from './topicsApi'
import { useTopicList } from './useTopicList'
import './forum.css'

type Dialog = { kind: 'create' } | { kind: 'edit'; topic: ForumTopic } | { kind: 'delete'; topic: ForumTopic } | null

export const topicHref = (chatId: string, topicId: string) =>
  `/chat/${encodeURIComponent(chatId)}/topic/${encodeURIComponent(topicId)}`

export function TopicListView({ api = topicsApi }: { api?: TopicsApi | undefined }) {
  const { chatId = '' } = useParams()
  const chat = useStore(chatListStore, selectChatById(chatId))
  const topics = useTopicList(api, chatId)
  const { list, scheduleReload, dropTopic, applyTopic } = topics
  const [dialog, setDialog] = useState<Dialog>(null)
  const [deleting, setDeleting] = useState(false)
  const now = useMinuteClock()

  const onFrame = useCallback(
    (frame: ServerFrame) => {
      if (frame.type === 'topic_updated' && frame.topic.deleted) dropTopic(frame.topic.id)
      else if (frame.type === 'topic_updated' || frame.type === 'broadcast' || frame.type === 'message_recalled') {
        scheduleReload()
      }
    },
    [dropTopic, scheduleReload],
  )
  const { connection } = useChatSession(chatId, { readCursor: false, onFrame })

  useEffect(() => {
    uiStore.getState().setActiveChat(chatId)
    return () => uiStore.getState().setActiveChat('')
  }, [chatId])

  const rows = useMemo(() => (list ? visibleTopics(list) : []), [list])
  const canManage = list?.can_manage ?? false
  const title = chat?.title || '…'
  const subtitle = CONNECTION_COPY[connection] ?? (list ? `${rows.length} 个话题` : '')

  const onAction = (topic: ForumTopic) => (action: TopicAction) => {
    if (action === 'edit') setDialog({ kind: 'edit', topic })
    else if (action === 'delete') setDialog({ kind: 'delete', topic })
    else void topics.perform(action, topic)
  }

  const confirmDelete = (topic: ForumTopic) => {
    setDeleting(true)
    void topics.remove(topic).then(() => {
      setDeleting(false)
      setDialog(null)
    })
  }

  return (
    <div className="tg-chat tg-forum">
      <header className="tg-chat__header">
        <MobileBackButton />
        <button type="button" className="tg-chat__identity" onClick={() => toggleChatInfo()} aria-label="查看会话信息">
          <Avatar label={title} initials={chat?.avatar_emoji || undefined} />
          <span className="tg-chat__titles">
            <span className="tg-chat__title">{title}</span>
            <span className="tg-chat__subtitle">{subtitle}</span>
          </span>
        </button>
        {list?.can_create ? (
          <Button variant="text" size="sm" className="tg-forum__create" onClick={() => setDialog({ kind: 'create' })}>
            新建话题
          </Button>
        ) : null}
      </header>
      {topics.error ? (
        <p className="tg-forum__error" role="alert">
          {topics.error}
        </p>
      ) : null}
      <ScrollArea className="tg-forum__body" orientation="vertical">
        {list === null ? (
          topics.error ? null : (
            <div className="tg-forum__loading">
              <Spinner label="正在加载话题" />
            </div>
          )
        ) : rows.length === 0 ? (
          <p className="tg-forum__empty">还没有话题</p>
        ) : (
          <ul className="tg-forum__list" aria-label="话题">
            {rows.map((row) => (
              <TopicRow
                key={row.topic.id}
                row={row}
                href={topicHref(chatId, row.topic.id)}
                canManage={canManage}
                now={now}
                onAction={onAction(row.topic)}
              />
            ))}
          </ul>
        )}
      </ScrollArea>
      {dialog?.kind === 'create' || dialog?.kind === 'edit' ? (
        <TopicDialog
          key={dialog.kind === 'edit' ? dialog.topic.id : 'create'}
          chatId={chatId}
          api={api}
          topic={dialog.kind === 'edit' ? dialog.topic : null}
          onClose={() => setDialog(null)}
          onSaved={(topic) => {
            applyTopic(topic)
            setDialog(null)
          }}
        />
      ) : null}
      {dialog?.kind === 'delete' ? (
        <Modal
          open
          size="sm"
          title="删除话题"
          description={`确定删除「${dialog.topic.title}」吗？话题中的所有消息都将被删除。`}
          onClose={() => !deleting && setDialog(null)}
          footer={
            <>
              <Button variant="text" disabled={deleting} onClick={() => setDialog(null)}>
                取消
              </Button>
              <Button variant="danger" loading={deleting} onClick={() => confirmDelete(dialog.topic)}>
                删除
              </Button>
            </>
          }
        />
      ) : null}
    </div>
  )
}

export default TopicListView
