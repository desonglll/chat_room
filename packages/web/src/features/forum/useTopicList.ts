/**
 * The topic list's data (TG-204): one `GET /topics` per open, a debounced refetch that
 * live frames trigger, and the row actions that answer a topic (upserted in place).
 */
import { useCallback, useEffect, useRef, useState } from 'react'
import type { ForumTopic, ForumTopicList, TopicsApi, UpdateTopicInput } from '@tg/core'
import { chatListStore } from '@tg/core'
import { browserClock } from '../../app/platform'
import type { TopicAction } from './topicListModel'
import { removeTopic, topicErrorText, upsertTopic } from './topicListModel'

export const TOPIC_REFRESH_DEBOUNCE_MS = 300

const PATCHES: Partial<Record<TopicAction, UpdateTopicInput>> = {
  pin: { is_pinned: true },
  unpin: { is_pinned: false },
  close: { is_closed: true },
  reopen: { is_closed: false },
  hide: { is_hidden: true },
  unhide: { is_hidden: false },
}

export interface TopicListState {
  list: ForumTopicList | null
  error: string
  /** Refetch soon; bursts of frames collapse into one request. */
  scheduleReload(): void
  applyTopic(topic: ForumTopic): void
  dropTopic(topicId: string): void
  /** The data actions (not edit / delete, which need a dialog first). */
  perform(action: TopicAction, topic: ForumTopic): Promise<void>
  remove(topic: ForumTopic): Promise<boolean>
}

export function useTopicList(api: TopicsApi, chatId: string): TopicListState {
  const [list, setList] = useState<ForumTopicList | null>(null)
  const [error, setError] = useState('')
  const timer = useRef<unknown>(null)
  const generation = useRef(0)

  const load = useCallback(() => {
    const at = generation.current
    api.list(chatId).then(
      (answer) => {
        if (at !== generation.current) return
        setList(answer)
        setError('')
      },
      (caught: unknown) => {
        if (at === generation.current) setError(topicErrorText(caught))
      },
    )
  }, [api, chatId])

  useEffect(() => {
    generation.current += 1
    setList(null)
    setError('')
    load()
    return () => {
      generation.current += 1
      if (timer.current !== null) browserClock.clearTimeout(timer.current)
      timer.current = null
    }
  }, [load])

  const scheduleReload = useCallback(() => {
    if (timer.current !== null) browserClock.clearTimeout(timer.current)
    timer.current = browserClock.setTimeout(() => {
      timer.current = null
      load()
    }, TOPIC_REFRESH_DEBOUNCE_MS)
  }, [load])

  const applyTopic = useCallback((topic: ForumTopic) => {
    setList((current) => (current ? upsertTopic(current, topic) : current))
  }, [])
  const dropTopic = useCallback((topicId: string) => {
    setList((current) => (current ? removeTopic(current, topicId) : current))
  }, [])

  const perform = useCallback(
    async (action: TopicAction, topic: ForumTopic) => {
      try {
        const patch = PATCHES[action]
        if (patch) applyTopic(await api.update(chatId, topic.id, patch))
        else if (action === 'mute' || action === 'unmute') {
          applyTopic(await api.setNotifications(chatId, topic.id, { muted: action === 'mute', muted_until: null }))
        } else if (action === 'read' && topic.last_message) {
          const result = await api.read(chatId, topic.id, topic.last_message.message_id)
          applyTopic({ ...topic, unread_count: result.unread_count })
          if (result.chat_read_advanced) chatListStore.getState().setUnreadCount(chatId, 0)
        }
        setError('')
      } catch (caught) {
        setError(topicErrorText(caught))
      }
    },
    [api, chatId, applyTopic],
  )

  const remove = useCallback(
    async (topic: ForumTopic) => {
      try {
        await api.remove(chatId, topic.id)
        dropTopic(topic.id)
        setError('')
        return true
      } catch (caught) {
        setError(topicErrorText(caught))
        return false
      }
    },
    [api, chatId, dropTopic],
  )

  return { list, error, scheduleReload, applyTopic, dropTopic, perform, remove }
}
