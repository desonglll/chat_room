/**
 * The row around a bubble: side, group spacing, avatar column, selection checkbox,
 * highlight flash, and the right-click / long-press menu. Content-agnostic — both the
 * message bubble and the upload placeholder sit in it.
 */
import type { ReactNode } from 'react'
import { Avatar, Checkbox, ContextMenu, type MenuItem } from '@tg/ui'
import type { MessageRenderContext } from './types'

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
  children,
}: {
  ctx: MessageRenderContext
  selectionMode: boolean
  reserveAvatar: boolean
  avatar: RowAvatar
  menuItems: readonly MenuItem[]
  onSelect?: (() => void) | undefined
  children: ReactNode
}) {
  const selectable = selectionMode && onSelect !== undefined
  const avatarColumn = !ctx.isOutgoing && (reserveAvatar || ctx.showAvatar)

  return (
    <div
      className="tg-message"
      data-side={ctx.isOutgoing ? 'out' : 'in'}
      data-group={ctx.groupPosition}
      data-selected={ctx.selected ? '' : undefined}
      data-highlighted={ctx.highlighted ? '' : undefined}
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
            aria-label={ctx.selected ? '取消选择消息' : '选择消息'}
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
      <ContextMenu items={menuItems} disabled={selectionMode} aria-label="消息操作" className="tg-message__menu-region">
        <div className="tg-message__column">{children}</div>
      </ContextMenu>
    </div>
  )
}
