/**
 * Mobile single-column layout: the way back from a chat to the list. Rendered inline as
 * the chat header's first child (TG-100); CSS shows it only below the mobile breakpoint.
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
