/**
 * Pure rules of the topic list (TG-204): which rows a viewer sees, the preview line, and
 * which row actions a viewer is offered. The server re-checks every action; this only
 * decides what to show.
 */
import type { ForumTopic, ForumTopicList } from '@tg/core'
import { ApiError, sortTopics } from '@tg/core'
import { t } from '../../i18n/index'

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
  if (!last) return { sender: '', text: topic.is_general ? '' : t('w.forum.9bc52b') }
  const text = last.content.replace(/\s+/gu, ' ').trim()
  return { sender: last.sender, text: text || t('w.forum.ee62cf') }
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
  get read() {
    return t('w.forum.6cc55e')
  },
  get edit() {
    return t('w.forum.5ff368')
  },
  get close() {
    return t('w.forum.007435')
  },
  get reopen() {
    return t('w.forum.1c2719')
  },
  get pin() {
    return t('w.forum.7bcf18')
  },
  get unpin() {
    return t('w.forum.cfb5cd')
  },
  get mute() {
    return t('w.forum.afdbd1')
  },
  get unmute() {
    return t('w.forum.59bc75')
  },
  get hide() {
    return t('w.forum.1993c9')
  },
  get unhide() {
    return t('w.forum.0fc3b6')
  },
  get delete() {
    return t('w.forum.e3b476')
  },
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
  get 400() {
    return t('w.forum.eec31b')
  },
  get 403() {
    return t('w.forum.428d1f')
  },
  get 404() {
    return t('w.forum.3499f4')
  },
  get 409() {
    return t('w.forum.2661c8')
  },
}

export function topicErrorText(error: unknown): string {
  if (error instanceof ApiError) return ERROR_TEXT[error.status] ?? t('w.forum.51d3cb')
  return t('w.forum.00b4a5')
}

/** The first visible character of a title, for the coloured glyph. */
export function topicInitial(title: string): string {
  return [...title.trim()][0]?.toUpperCase() ?? '#'
}
