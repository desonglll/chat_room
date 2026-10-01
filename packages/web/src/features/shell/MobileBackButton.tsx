/**
 * Mobile single-column layout: the way back from a chat to the list. Rendered inline as
 * the chat header's first child (TG-100); CSS shows it only below the mobile breakpoint.
 */
import { useNavigate } from 'react-router-dom'
import { IconButton } from '@tg/ui'
import { ChatListIcon } from '../chatList/chatListIcons'
import { t } from '../../i18n/index'

export function MobileBackButton({ className }: { className?: string }) {
  const navigate = useNavigate()
  return (
    <IconButton
      label={t('w.shell.dcdc8d')}
      variant="plain"
      className={className ? `tg-mobile-back ${className}` : 'tg-mobile-back'}
      onClick={() => void navigate('/')}
    >
      <ChatListIcon name="back" size={22} />
    </IconButton>
  )
}
