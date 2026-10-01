/**
 * One topic row (TG-204): icon, title (+ closed lock, muted glyph), time, the last
 * message as "sender: text", and the unread badge (grey when muted) or the pin glyph.
 * The row menu (right click / long press / Shift+F10) carries the permitted actions.
 */
import type { MenuItem } from '@tg/ui'
import { Badge, ContextMenu } from '@tg/ui'
import { Link } from 'react-router-dom'
import { ChatListIcon } from '../chatList/chatListIcons'
import { formatChatListTime } from '../chatList/chatTime'
import { TopicIcon } from './TopicIcon'
import type { TopicAction, TopicRowView } from './topicListModel'
import { TOPIC_ACTION_LABEL, topicActions, topicPreview } from './topicListModel'
import { t } from '../../i18n/index'

export interface TopicRowProps {
  row: TopicRowView
  href: string
  canManage: boolean
  now: Date
  onAction?: ((action: TopicAction) => void) | undefined
}

export function LockGlyph({ size = 14 }: { size?: number }) {
  return (
    <svg viewBox="0 0 24 24" width={size} height={size} aria-hidden="true" focusable="false">
      <rect x="5" y="10.5" width="14" height="10" rx="2" fill="currentColor" />
      <path d="M8 10.5V8a4 4 0 0 1 8 0v2.5" fill="none" stroke="currentColor" strokeWidth="2" />
    </svg>
  )
}

export function TopicRow({ row, href, canManage, now, onAction }: TopicRowProps) {
  const { topic, dimmed } = row
  const preview = topicPreview(topic)
  const activity = topic.last_message?.created_at ?? topic.created_at
  const items: MenuItem[] = onAction
    ? topicActions(topic, canManage).map((action) => ({
        id: action,
        label: TOPIC_ACTION_LABEL[action],
        danger: action === 'delete',
        separatorBefore: action === 'delete',
        onSelect: () => onAction(action),
      }))
    : []

  return (
    <li className="tg-topicrow__item">
      <ContextMenu
        items={items}
        disabled={items.length === 0}
        aria-label={t('w.forum.4d6376')}
        className="tg-topicrow__menu"
      >
        <Link
          to={href}
          className="tg-topicrow"
          data-dimmed={dimmed || undefined}
          data-muted={topic.muted || undefined}
          data-closed={topic.is_closed || undefined}
        >
          <TopicIcon title={topic.title} emoji={topic.icon_emoji} color={topic.icon_color} general={topic.is_general} />
          <span className="tg-topicrow__body">
            <span className="tg-topicrow__top">
              <span className="tg-topicrow__title">{topic.title}</span>
              {topic.is_closed ? (
                <span className="tg-topicrow__glyph" role="img" aria-label={t('w.forum.f62876')}>
                  <LockGlyph />
                </span>
              ) : null}
              {topic.muted ? (
                <span className="tg-topicrow__glyph" role="img" aria-label={t('w.forum.a074ec')}>
                  <ChatListIcon name="muted" size={14} />
                </span>
              ) : null}
              <span className="tg-topicrow__time">{formatChatListTime(activity, now)}</span>
            </span>
            <span className="tg-topicrow__bottom">
              <span className="tg-topicrow__preview">
                {preview.sender ? <span className="tg-topicrow__sender">{preview.sender}: </span> : null}
                {preview.text}
              </span>
              {topic.unread_count > 0 ? (
                <Badge count={topic.unread_count} variant={topic.muted ? 'muted' : 'accent'} />
              ) : topic.is_pinned ? (
                <span className="tg-topicrow__glyph" role="img" aria-label={t('w.forum.d7d970')}>
                  <ChatListIcon name="pin" size={16} />
                </span>
              ) : null}
            </span>
          </span>
        </Link>
      </ContextMenu>
    </li>
  )
}
