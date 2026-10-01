/**
 * The row around a bubble: side, group spacing, avatar column, selection checkbox,
 * highlight flash, and the right-click / long-press menu. Content-agnostic — both the
 * message bubble and the upload placeholder sit in it.
 */
import type { ReactNode } from 'react'
import { useState } from 'react'
import { Avatar, Checkbox, ContextMenu, type MenuItem } from '@tg/ui'
import type { MessageRenderContext } from './types'
import { t } from '../../i18n/index'

/** A message younger than this when its row mounts arrived live and rises in (TG-108). */
export const FRESH_MESSAGE_MS = 3000

/** Whether a row mounting at `mountedAt` shows a message that just arrived (not history). */
export function isFreshMessage(sentAt: string | undefined, mountedAt: number): boolean {
  if (sentAt === undefined) return true
  const sent = Date.parse(sentAt)
  return Number.isFinite(sent) && mountedAt - sent < FRESH_MESSAGE_MS
}

export interface RowAvatar {
  label: string
  emoji: string
}

export function MessageRow({
  ctx,
  selectionMode,
  reserveAvatar,
  avatar,
  menuItems,
  onSelect,
  sentAt,
  children,
}: {
  ctx: MessageRenderContext
  selectionMode: boolean
  reserveAvatar: boolean
  avatar: RowAvatar
  menuItems: readonly MenuItem[]
  onSelect?: (() => void) | undefined
  /** The message's timestamp; omitted for a local upload, which is always fresh. */
  sentAt?: string | undefined
  children: ReactNode
}) {
  // Decided once at mount: history pages and re-renders never replay the entrance.
  const [fresh] = useState(() => isFreshMessage(sentAt, Date.now()))
  const selectable = selectionMode && onSelect !== undefined
  const avatarColumn = !ctx.isOutgoing && (reserveAvatar || ctx.showAvatar)

  return (
    <div
      className="tg-message"
      data-side={ctx.isOutgoing ? 'out' : 'in'}
      data-group={ctx.groupPosition}
      data-selected={ctx.selected ? '' : undefined}
      data-highlighted={ctx.highlighted ? '' : undefined}
      data-fresh={fresh ? '' : undefined}
      data-selection-mode={selectionMode ? '' : undefined}
      onClick={selectable ? onSelect : undefined}
    >
      {selectionMode ? (
        <span className="tg-message__check" onClick={(event) => event.stopPropagation()}>
          <Checkbox
            shape="round"
            checked={ctx.selected}
            disabled={onSelect === undefined}
            onCheckedChange={() => onSelect?.()}
            aria-label={ctx.selected ? t('w.message.c1c53d') : t('w.message.2b0443')}
          />
        </span>
      ) : null}
      {avatarColumn ? (
        <div className="tg-message__avatar">
          {ctx.showAvatar ? (
            <Avatar size="sm" label={avatar.label} initials={avatar.emoji === '' ? undefined : avatar.emoji} />
          ) : null}
        </div>
      ) : null}
      <ContextMenu
        items={menuItems}
        disabled={selectionMode}
        aria-label={t('w.message.9f2251')}
        className="tg-message__menu-region"
      >
        <div className="tg-message__column">{children}</div>
      </ContextMenu>
    </div>
  )
}
