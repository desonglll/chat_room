/**
 * A chat row with its archive affordances (TG-502): the context menu (right click, touch
 * long-press, Shift+F10) and the touch swipe-left gesture. Both call the same `onToggleArchive`.
 */
import type { ReactNode } from 'react'
import type { MenuItem } from '@tg/ui'
import { ContextMenu } from '@tg/ui'
import { ChatListIcon } from './chatListIcons'
import { SwipeArchive } from './SwipeArchive'

export interface ArchivableChatRowProps {
  archived: boolean
  /** The avatar-only sidebar column: menu yes, swipe no (it is a desktop layout). */
  collapsed: boolean
  onToggleArchive: () => void
  children: ReactNode
}

export function archiveMenuItems(archived: boolean, onToggleArchive: () => void): MenuItem[] {
  return [
    {
      id: archived ? 'unarchive' : 'archive',
      label: archived ? '取消归档' : '归档',
      textValue: archived ? '取消归档' : '归档',
      icon: <ChatListIcon name={archived ? 'unarchive' : 'archive'} size={20} />,
      onSelect: onToggleArchive,
    },
  ]
}

export function ArchivableChatRow({ archived, collapsed, onToggleArchive, children }: ArchivableChatRowProps) {
  return (
    <ContextMenu items={archiveMenuItems(archived, onToggleArchive)} aria-label="会话菜单" className="tg-chatrow-menu">
      <SwipeArchive archived={archived} onCommit={onToggleArchive} disabled={collapsed}>
        {children}
      </SwipeArchive>
    </ContextMenu>
  )
}
