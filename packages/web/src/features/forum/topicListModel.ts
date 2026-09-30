/**
 * Pure rules of the topic list (TG-204): which rows a viewer sees, the preview line, and
 * which row actions a viewer is offered. The server re-checks every action; this only
 * decides what to show.
 */
import type { ForumTopic, ForumTopicList } from '@tg/core'
import { ApiError, sortTopics } from '@tg/core'

export interface TopicRowView {
  topic: ForumTopic
  /** A hidden General shown to a manager only, drawn dimmed. */
  dimmed: boolean
}

/** Rows in server order; a hidden General is shown to managers only (dimmed). */
export function visibleTopics(list: Pick<ForumTopicList, 'topics' | 'can_manage'>): TopicRowView[] {
  return sortTopics(list.topics).flatMap((topic): TopicRowView[] => {
    if (topic.is_general && topic.is_hidden) return list.can_manage ? [{ topic, dimmed: true }] : []
    return [{ topic, dimmed: false }]
  })
}

/** Replace (or add) one topic after a mutation answered it, keeping the server order. */
export function upsertTopic(list: ForumTopicList, topic: ForumTopic): ForumTopicList {
  const others = list.topics.filter((row) => row.id !== topic.id)
  return { ...list, topics: sortTopics([...others, topic]) }
}

export function removeTopic(list: ForumTopicList, topicId: string): ForumTopicList {
  return { ...list, topics: list.topics.filter((row) => row.id !== topicId) }
}

export interface TopicPreview {
  sender: string
  text: string
}

export function topicPreview(topic: ForumTopic): TopicPreview {
  const last = topic.last_message
  if (!last) return { sender: '', text: topic.is_general ? '' : '话题已创建' }
  const text = last.content.replace(/\s+/gu, ' ').trim()
  return { sender: last.sender, text: text || '[媒体]' }
}

export type TopicAction =
  | 'read'
  | 'edit'
  | 'close'
  | 'reopen'
  | 'pin'
  | 'unpin'
  | 'mute'
  | 'unmute'
  | 'hide'
  | 'unhide'
  | 'delete'

export const TOPIC_ACTION_LABEL: Record<TopicAction, string> = {
  read: '标记为已读',
  edit: '编辑话题',
  close: '关闭话题',
  reopen: '重新打开话题',
  pin: '置顶',
  unpin: '取消置顶',
  mute: '静音',
  unmute: '取消静音',
  hide: '隐藏 General',
  unhide: '显示 General',
  delete: '删除话题',
}

/** Telegram's row menu: edit/close for the creator or a manager, the rest for managers. */
export function topicActions(topic: ForumTopic, canManage: boolean): TopicAction[] {
  const actions: TopicAction[] = []
  if (topic.unread_count > 0 && topic.last_message) actions.push('read')
  if (canManage) actions.push(topic.is_pinned ? 'unpin' : 'pin')
  actions.push(topic.muted ? 'unmute' : 'mute')
  if (topic.can_edit) actions.push('edit', topic.is_closed ? 'reopen' : 'close')
  if (canManage && topic.is_general) actions.push(topic.is_hidden ? 'unhide' : 'hide')
  if (canManage && !topic.is_general) actions.push('delete')
  return actions
}

const ERROR_TEXT: Record<number, string> = {
  400: '话题信息无效',
  403: '没有权限执行此操作',
  404: '话题不存在或已被删除',
  409: '此群组未开启话题',
}

export function topicErrorText(error: unknown): string {
  if (error instanceof ApiError) return ERROR_TEXT[error.status] ?? '操作失败，请重试'
  return '网络异常，请重试'
}

/** The first visible character of a title, for the coloured glyph. */
export function topicInitial(title: string): string {
  return [...title.trim()][0]?.toUpperCase() ?? '#'
}
