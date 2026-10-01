/** One exception list of the rule editor: its accounts with remove buttons, plus "add". */
import { Avatar, Button, IconButton } from '@tg/ui'
import type { PrivacyUser } from '@tg/core'
import { privacyUserName } from './privacyCopy'
import { t } from '../../../i18n/index'

export interface ExceptionListProps {
  title: string
  users: readonly PrivacyUser[]
  disabled: boolean
  onAdd: () => void
  onRemove: (userId: string) => void
}

export function ExceptionList({ title, users, disabled, onAdd, onRemove }: ExceptionListProps) {
  return (
    <section className="tg-privacy__section" aria-label={title}>
      <h3 className="tg-privacy__section-title">{title}</h3>
      <ul className="tg-privacy__users">
        {users.map((user) => {
          const name = privacyUserName(user)
          return (
            <li key={user.id} className="tg-privacy__user">
              <Avatar label={name} size="sm" />
              <span className="tg-privacy__user-name">{name}</span>
              <IconButton
                label={t('w.settings.6a13fa', name)}
                size="sm"
                disabled={disabled}
                onClick={() => onRemove(user.id)}
              >
                <span aria-hidden="true">×</span>
              </IconButton>
            </li>
          )
        })}
      </ul>
      <Button variant="text" size="sm" disabled={disabled} onClick={onAdd}>
        {t('w.settings.4f965d')}
      </Button>
    </section>
  )
}
