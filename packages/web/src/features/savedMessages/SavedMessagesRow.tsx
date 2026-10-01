/** TG-503: the pinned «收藏夹» row at the top of the chat list (Telegram's Saved Messages). */
import { useMatch, useNavigate } from 'react-router-dom'
import { Avatar } from '@tg/ui'
import { t } from '../../i18n/index'

export function SavedMessagesRow({ collapsed }: { collapsed: boolean }) {
  const navigate = useNavigate()
  const active = useMatch('/saved') !== null
  return (
    <button
      type="button"
      className="tg-saved-row"
      data-active={active || undefined}
      aria-current={active ? 'page' : undefined}
      aria-label={t('w.savedMessages.e6f497')}
      onClick={() => void navigate('/saved')}
    >
      <Avatar label={t('w.savedMessages.e6f497')} initials="🔖" size="md" />
      {collapsed ? null : (
        <span className="tg-saved-row__text">
          <span className="tg-saved-row__title">{t('w.savedMessages.e6f497')}</span>
          <span className="tg-saved-row__subtitle">{t('w.savedMessages.aa3992')}</span>
        </span>
      )}
    </button>
  )
}
