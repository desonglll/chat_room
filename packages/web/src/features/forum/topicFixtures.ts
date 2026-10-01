/** Test fixtures for the forum feature (not a test file itself). */
import type { ForumTopic } from '@tg/core'
import { TOPIC_COLORS } from '@tg/core'
import { t } from '../../i18n/index'

export function makeTopic(id: string, extra: Partial<ForumTopic> = {}): ForumTopic {
  return {
    id,
    chat_id: 'chat-1',
    is_general: false,
    title: t('w.forum.c32a25', id),
    icon_emoji: '',
    icon_custom_emoji_id: null,
    icon_color: TOPIC_COLORS[0],
    is_pinned: false,
    pinned_at: null,
    is_closed: false,
    is_hidden: false,
    creator_id: null,
    created_at: '2026-10-01T00:00:00Z',
    last_message: null,
    unread_count: 0,
    muted: false,
    muted_until: null,
    can_edit: false,
    ...extra,
  }
}
