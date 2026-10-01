/**
 * The "已归档的对话" entry at the top of the main list (TG-502). Two looks, Telegram's:
 *  - `collapsed` (default): a thin row with a stack of the archived chats' avatars;
 *  - `expanded`: a chat-height row, archive glyph avatar, the archived chats' names as preview
 *    (unread ones first and emphasised).
 * Its context menu switches between the two or hides the row into the hamburger menu. The badge
 * is the unread sum over archived non-muted chats, grey when only muted chats have unread
 * (`archiveRules.ts::archiveBadge`).
 */
import type { MenuItem } from '@tg/ui'
import { Avatar, Badge, ContextMenu } from '@tg/ui'
import type { ConversationSummary } from '@tg/core'
import { isConversationMuted } from '@tg/core'
import type { ArchiveBadge } from './archiveRules'
import type { ArchiveRowMode } from './archiveRowMode'
import { avatarParts } from './chatRowModel'
import { ChatListIcon } from './chatListIcons'
import { t } from '../../i18n/index'

/** «已归档的对话» in the current language. */
export const archiveTitle = () => t('w.chatList.archiveTitle')

export interface ArchiveRowProps {
  mode: Exclude<ArchiveRowMode, 'hidden'>
  /** Newest archived chats (unread first), for the avatar stack and the preview line. */
  previewChats: readonly ConversationSummary[]
  count: number
  badge: ArchiveBadge
  /** For the unread-and-not-muted emphasis in the expanded preview. */
  now: number
  /** The sidebar's avatar-only column. */
  collapsed?: boolean | undefined
  onOpen: () => void
  onModeChange: (mode: ArchiveRowMode) => void
}

const title = (conversation: ConversationSummary) => conversation.alias || conversation.title

function AvatarStack({ chats }: { chats: readonly ConversationSummary[] }) {
  return (
    <span className="tg-archive__stack" aria-hidden="true">
      {chats.slice(0, 3).map((conversation) => {
        const { src, initials } = avatarParts(conversation.avatar_emoji)
        return (
          <Avatar
            key={conversation.room_id}
            className="tg-archive__stack-avatar"
            label={title(conversation)}
            initials={initials}
            src={src}
            size={24}
          />
        )
      })}
    </span>
  )
}

function Names({ chats, now }: { chats: readonly ConversationSummary[]; now: number }) {
  return (
    <span className="tg-chatrow__preview tg-archive__names">
      {chats.map((conversation, index) => (
        <span key={conversation.room_id}>
          {index > 0 ? ', ' : null}
          <span
            className={
              conversation.unread_count > 0 && !isConversationMuted(conversation, now)
                ? 'tg-archive__name--unread'
                : undefined
            }
          >
            {title(conversation)}
          </span>
        </span>
      ))}
    </span>
  )
}

export function ArchiveRow({
  mode,
  previewChats,
  count,
  badge,
  now,
  collapsed = false,
  onOpen,
  onModeChange,
}: ArchiveRowProps) {
  const items: MenuItem[] = [
    mode === 'collapsed'
      ? { id: 'expand', label: t('w.chatList.b0e248'), onSelect: () => onModeChange('expanded') }
      : { id: 'collapse', label: t('w.chatList.e9132e'), onSelect: () => onModeChange('collapsed') },
    { id: 'hide', label: t('w.chatList.45fe84'), onSelect: () => onModeChange('hidden') },
  ]
  const badgeNode =
    badge.count > 0 ? (
      <Badge
        count={badge.count}
        variant={badge.muted ? 'muted' : 'accent'}
        size={collapsed || mode === 'collapsed' ? 'sm' : undefined}
        className="tg-chatrow__badge"
      />
    ) : null
  const label = t(
    'w.chatList.01c31b',
    archiveTitle(),
    count,
    badge.count > 0 ? t('w.chatList.unreadSuffix', badge.count) : '',
  )

  if (collapsed) {
    return (
      <button
        type="button"
        className="tg-chatrow tg-chatrow--collapsed tg-archive"
        onClick={onOpen}
        title={archiveTitle()}
        aria-label={label}
      >
        <span className="tg-chatrow__archive-avatar tg-archive__glyph" aria-hidden="true">
          <ChatListIcon name="archive" size={26} />
          {badgeNode ? <span className="tg-archive__glyph-badge">{badgeNode}</span> : null}
        </span>
      </button>
    )
  }

  return (
    <ContextMenu items={items} aria-label={t('w.chatList.f854b8')} className="tg-archive__menu-region">
      {mode === 'collapsed' ? (
        <button
          type="button"
          className="tg-archive tg-archive--collapsed"
          onClick={onOpen}
          aria-label={label}
          data-mode="collapsed"
        >
          <AvatarStack chats={previewChats} />
          <span className="tg-archive__title">{archiveTitle()}</span>
          {badgeNode}
        </button>
      ) : (
        <button
          type="button"
          className="tg-chatrow tg-archive tg-archive--expanded"
          onClick={onOpen}
          aria-label={label}
          data-mode="expanded"
        >
          <span className="tg-chatrow__archive-avatar" aria-hidden="true">
            <ChatListIcon name="archive" size={26} />
          </span>
          <span className="tg-chatrow__body">
            <span className="tg-chatrow__top">
              <span className="tg-chatrow__title">{archiveTitle()}</span>
            </span>
            <span className="tg-chatrow__bottom">
              <Names chats={previewChats} now={now} />
              {badgeNode}
            </span>
          </span>
        </button>
      )}
    </ContextMenu>
  )
}
