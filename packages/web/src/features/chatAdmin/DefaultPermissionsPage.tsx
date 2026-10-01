/** The group's default permissions: Telegram's "成员权限" — what every ordinary member may do. */
import { useState } from 'react'
import type { ChatPermissionsView } from '@tg/core'
import { Button } from '@tg/ui'
import { memberOptions, toggleMemberPermission } from './chatAdminModel'
import { PermissionChecklist } from './PermissionChecklist'
import { t } from '../../i18n/index'

export interface DefaultPermissionsPageProps {
  view: ChatPermissionsView
  busy: boolean
  /** Read-only for a viewer without `members.ban`. */
  editable: boolean
  onSave(permissions: string[]): void
}

export function DefaultPermissionsPage({ view, busy, editable, onSave }: DefaultPermissionsPageProps) {
  const [allowed, setAllowed] = useState<Set<string>>(() => new Set(view.default_permissions))
  const options = memberOptions(view.registry, allowed)
  const changed =
    allowed.size !== view.default_permissions.length || view.default_permissions.some((key) => !allowed.has(key))

  return (
    <div className="tg-chatadmin__editor">
      <PermissionChecklist
        title={t('w.chatAdmin.35c46a')}
        options={options}
        disabled={busy || !editable}
        onToggle={(key, checked) => setAllowed((set) => toggleMemberPermission(set, key, checked))}
      />
      <p className="tg-chatadmin__note">{t('w.chatAdmin.433e4f')}</p>
      {editable ? (
        <div className="tg-chatadmin__actions">
          <Button
            loading={busy}
            disabled={busy || !changed}
            onClick={() => onSave(options.filter((o) => o.checked).map((o) => o.key))}
          >
            {t('w.chatAdmin.fadf24')}
          </Button>
        </div>
      ) : null}
    </div>
  )
}
