/**
 * One chat row, every Telegram state: avatar (+ online dot), title (+ muted glyph),
 * own-message ticks + time, preview line (sender prefix / media glyph / draft marker /
 * typing), unread badge (grey when muted) or pin glyph. Plain props only — the pane
 * adapts store data into `ChatRowModel`, and presence arrives as `isOnline`/`typingText`
 * (TG-107 seam). `collapsed` is the avatar-only sidebar column.
 */
import type { MouseEvent } from 'react'
import { Avatar, Badge } from '@tg/ui'
import type { ChatPreview } from './chatPreview'
import { MEDIA_LABEL } from './chatPreview'
import type { ChatRowModel } from './chatRowModel'
import { ChatListIcon } from './chatListIcons'
import { t } from '../../i18n/index'

export interface ChatListRowProps {
  model: ChatRowModel
  href: string
  active: boolean
  collapsed?: boolean | undefined
  /** Presence seam: `undefined` = unknown (no dot). Ignored unless the chat is private. */
  isOnline?: boolean | undefined
  /** Presence seam: replaces the preview line while set. */
  typingText?: string | null | undefined
  onOpen?: ((chatId: string, event: MouseEvent<HTMLAnchorElement>) => void) | undefined
}

function PreviewLine({ preview, typingText }: { preview: ChatPreview; typingText: string | null | undefined }) {
  if (typingText) {
    return <span className="tg-chatrow__preview tg-chatrow__preview--typing">{typingText}</span>
  }
  if (preview.kind === 'typing') {
    return <span className="tg-chatrow__preview tg-chatrow__preview--typing">{preview.text}</span>
  }
  if (preview.kind === 'draft') {
    return (
      <span className="tg-chatrow__preview">
        <span className="tg-chatrow__draft">{t('w.chatList.79d1a2')} </span>
        {preview.text}
      </span>
    )
  }
  if (preview.kind === 'empty') {
    return <span className="tg-chatrow__preview tg-chatrow__preview--empty">{preview.text}</span>
  }
  return (
    <span className={`tg-chatrow__preview${preview.recalled ? ' tg-chatrow__preview--recalled' : ''}`}>
      {preview.sender ? <span className="tg-chatrow__sender">{preview.sender}: </span> : null}
      {preview.media ? (
        <span className="tg-chatrow__media" role="img" aria-label={MEDIA_LABEL[preview.media]}>
          <ChatListIcon name={preview.media} size={16} />
        </span>
      ) : null}
      {preview.text}
    </span>
  )
}

export function ChatListRow({
  model,
  href,
  active,
  collapsed = false,
  isOnline,
  typingText,
  onOpen,
}: ChatListRowProps) {
  const online = model.showsPresence && isOnline === true
  const badgeVariant = model.muted ? 'muted' : 'accent'
  const unread = model.unreadCount > 0

  return (
    <a
      href={href}
      className={`tg-chatrow${active ? ' tg-chatrow--active' : ''}${collapsed ? ' tg-chatrow--collapsed' : ''}`}
      aria-current={active ? 'page' : undefined}
      title={collapsed ? model.title : undefined}
      data-muted={model.muted || undefined}
      data-pinned={model.pinned || undefined}
      onClick={onOpen ? (event) => onOpen(model.chatId, event) : undefined}
    >
      <Avatar
        label={model.avatarLabel}
        initials={model.avatarInitials}
        src={model.avatarSrc}
        online={online}
        badge={collapsed && unread ? <Badge count={model.unreadCount} variant={badgeVariant} size="sm" /> : undefined}
      />
      {collapsed ? null : (
        <span className="tg-chatrow__body">
          <span className="tg-chatrow__top">
            <span className="tg-chatrow__title">{model.title}</span>
            {model.muted ? (
              <span className="tg-chatrow__muted" role="img" aria-label={t('w.chatList.a074ec')}>
                <ChatListIcon name="muted" size={14} />
              </span>
            ) : null}
            <span className="tg-chatrow__meta">
              {model.outgoing ? (
                <span
                  className={`tg-chatrow__ticks tg-chatrow__ticks--${model.outgoing}`}
                  role="img"
                  aria-label={model.outgoing === 'read' ? t('w.chatList.642ec8') : t('w.chatList.afb629')}
                >
                  <ChatListIcon name={model.outgoing === 'read' ? 'checks' : 'check'} size={16} />
                </span>
              ) : null}
              <span className="tg-chatrow__time">{model.time}</span>
            </span>
          </span>
          <span className="tg-chatrow__bottom">
            <PreviewLine preview={model.preview} typingText={typingText} />
            {unread ? (
              <Badge count={model.unreadCount} variant={badgeVariant} className="tg-chatrow__badge" />
            ) : model.pinned ? (
              <span className="tg-chatrow__pin" role="img" aria-label={t('w.chatList.d7d970')}>
                <ChatListIcon name="pin" size={16} />
              </span>
            ) : null}
          </span>
        </span>
      )}
    </a>
  )
}
