/**
 * Mobile single-column layout: the way back from a chat to the list. Rendered by the
 * shell over the chat header's leading edge (CSS shows it only below the mobile
 * breakpoint); a chat header may render it inline instead (integration patch list).
 */
import { useNavigate } from 'react-router-dom'
import { IconButton } from '@tg/ui'
import { ChatListIcon } from '../chatList/chatListIcons'

export function MobileBackButton({ className }: { className?: string }) {
  const navigate = useNavigate()
  return (
    <IconButton
      label="返回会话列表"
      variant="plain"
      className={className ? `tg-mobile-back ${className}` : 'tg-mobile-back'}
      onClick={() => void navigate('/')}
    >
      <ChatListIcon name="back" size={22} />
    </IconButton>
  )
}
