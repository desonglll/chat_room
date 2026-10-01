/**
 * The create / edit topic form (TG-204): title (1–128 chars, trimmed), an optional emoji
 * icon, and one of Telegram's six colours for the glyph drawn when no emoji is chosen.
 */
import type { CreateTopicInput, ForumTopic, UpdateTopicInput } from '@tg/core'
import { MAX_TOPIC_TITLE_CHARS, TOPIC_COLORS } from '@tg/core'
import { t } from '../../i18n/index'

/** A curated set of Telegram's topic icons. */
export const TOPIC_EMOJI = [
  '📰',
  '💡',
  '❗',
  '❓',
  '📝',
  '📌',
  '📣',
  '📅',
  '💬',
  '🎮',
  '🎵',
  '🎨',
  '📚',
  '💼',
  '💰',
  '🏆',
  '🔥',
  '⭐',
  '❤️',
  '🎉',
  '🍕',
  '☕',
  '✈️',
  '🏠',
  '⚽',
  '📷',
  '🎬',
  '💻',
  '🛠️',
  '🤖',
] as const

export interface TopicForm {
  title: string
  /** `''` = no emoji (coloured glyph). */
  emoji: string
  color: number
}

export function initialTopicForm(topic?: ForumTopic | null, seed = Math.random()): TopicForm {
  if (topic) return { title: topic.title, emoji: topic.icon_emoji, color: topic.icon_color }
  const index = Math.floor(seed * TOPIC_COLORS.length) % TOPIC_COLORS.length
  return { title: '', emoji: '', color: TOPIC_COLORS[index] ?? TOPIC_COLORS[0] }
}

export const titleLength = (title: string): number => [...title.trim()].length

/** Null when valid, else the message to show. */
export function validateTopicForm(form: TopicForm): string | null {
  const length = titleLength(form.title)
  if (length === 0) return t('w.forum.3e9d49')
  if (length > MAX_TOPIC_TITLE_CHARS) return t('w.forum.8c9a43', MAX_TOPIC_TITLE_CHARS)
  if (!(TOPIC_COLORS as readonly number[]).includes(form.color)) return t('w.forum.e1fe1e')
  return null
}

export function topicCreateInput(form: TopicForm): CreateTopicInput {
  return {
    title: form.title.trim(),
    icon_color: form.color,
    ...(form.emoji ? { icon_emoji: form.emoji } : {}),
  }
}

/** Only the changed fields; an empty patch means nothing to save. */
export function topicPatch(form: TopicForm, topic: ForumTopic): UpdateTopicInput {
  const patch: UpdateTopicInput = {}
  const title = form.title.trim()
  if (title !== topic.title) patch.title = title
  // General keeps its fixed "#" icon: only its title is editable (Telegram).
  if (topic.is_general) return patch
  if (form.emoji !== topic.icon_emoji) patch.icon_emoji = form.emoji
  if (form.color !== topic.icon_color) patch.icon_color = form.color
  return patch
}
