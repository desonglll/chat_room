/**
 * The "archived chats" entry at the top of the main list. Archive behaviour (moving
 * chats in and out, auto-archive) is TG-502's; this row only opens the folder view.
 * Its badge is always grey: archived chats are muted by convention.
 */
import { Badge } from '@tg/ui'
import { ChatListIcon } from './chatListIcons'

export interface ArchiveRowProps {
  count: number
  unread: number
  collapsed?: boolean | undefined
  onOpen: () => void
}

export function ArchiveRow({ count, unread, collapsed = false, onOpen }: ArchiveRowProps) {
  return (
    <button
      type="button"
      className={`tg-chatrow tg-chatrow--archive${collapsed ? ' tg-chatrow--collapsed' : ''}`}
      onClick={onOpen}
      title={collapsed ? '已归档会话' : undefined}
    >
      <span className="tg-chatrow__archive-avatar" aria-hidden="true">
        <ChatListIcon name="archive" size={26} />
      </span>
      {collapsed ? null : (
        <span className="tg-chatrow__body">
          <span className="tg-chatrow__top">
            <span className="tg-chatrow__title">已归档会话</span>
          </span>
          <span className="tg-chatrow__bottom">
            <span className="tg-chatrow__preview tg-chatrow__preview--empty">{count} 个会话</span>
            {unread > 0 ? <Badge count={unread} variant="muted" className="tg-chatrow__badge" /> : null}
          </span>
        </span>
      )}
    </button>
  )
}
