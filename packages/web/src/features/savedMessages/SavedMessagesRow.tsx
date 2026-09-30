/** TG-503: the pinned «收藏夹» row at the top of the chat list (Telegram's Saved Messages). */
import { useMatch, useNavigate } from 'react-router-dom'
import { Avatar } from '@tg/ui'

export function SavedMessagesRow({ collapsed }: { collapsed: boolean }) {
  const navigate = useNavigate()
  const active = useMatch('/saved') !== null
  return (
    <button
      type="button"
      className="tg-saved-row"
      data-active={active || undefined}
      aria-current={active ? 'page' : undefined}
      aria-label="收藏夹"
      onClick={() => void navigate('/saved')}
    >
      <Avatar label="收藏夹" initials="🔖" size="md" />
      {collapsed ? null : (
        <span className="tg-saved-row__text">
          <span className="tg-saved-row__title">收藏夹</span>
          <span className="tg-saved-row__subtitle">保存的消息与笔记</span>
        </span>
      )}
    </button>
  )
}
