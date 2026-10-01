/**
 * Appoint an administrator or edit one: Telegram's "管理员权限" page — one checkbox per
 * right plus a custom title. Rights the actor lacks are shown disabled (the server refuses
 * them anyway). Dismissal is offered for an existing administrator when the viewer owns the chat.
 */
import { useState } from 'react'
import type { ChatMemberEntry, ChatPermissionsView } from '@tg/core'
import { MAX_ADMIN_TITLE_CHARS } from '@tg/core'
import { Button, TextField } from '@tg/ui'
import { adminOptions, defaultAdminSelection, memberName } from './chatAdminModel'
import { PermissionChecklist } from './PermissionChecklist'
import { t } from '../../i18n/index'

export interface AdminEditorProps {
  target: ChatMemberEntry
  actor: ChatPermissionsView
  busy: boolean
  onSave(permissions: string[], title: string): void
  onDismiss?: (() => void) | undefined
}

export function AdminEditor({ target, actor, busy, onSave, onDismiss }: AdminEditorProps) {
  const existing = target.role === 'admin'
  const [selected, setSelected] = useState<Set<string>>(() =>
    existing ? new Set(target.admin_rights ?? []) : defaultAdminSelection(actor),
  )
  const [title, setTitle] = useState(target.custom_title)
  const options = adminOptions(actor.registry, selected, actor)
  const tooLong = [...title.trim()].length > MAX_ADMIN_TITLE_CHARS

  return (
    <div className="tg-chatadmin__editor">
      <p className="tg-chatadmin__lead">
        {existing ? t('w.chatAdmin.a7f814') : t('w.chatAdmin.252767')} <strong>{memberName(target)}</strong>{' '}
        {t('w.chatAdmin.d8ea06')}
      </p>
      <PermissionChecklist
        title={t('w.chatAdmin.d10ddd')}
        options={options}
        disabled={busy}
        onToggle={(key, checked) =>
          setSelected((current) => {
            const next = new Set(current)
            if (checked) next.add(key)
            else next.delete(key)
            return next
          })
        }
      />
      <section className="tg-chatadmin__section">
        <TextField
          label={t('w.chatAdmin.e4ae1a')}
          value={title}
          maxLength={MAX_ADMIN_TITLE_CHARS * 2}
          hint={t('w.chatAdmin.601fcf')}
          error={tooLong ? t('w.chatAdmin.c2e39f', MAX_ADMIN_TITLE_CHARS) : undefined}
          onChange={(event) => setTitle(event.target.value)}
          fullWidth
        />
      </section>
      <div className="tg-chatadmin__actions">
        {existing && onDismiss ? (
          <Button variant="danger" disabled={busy} onClick={onDismiss}>
            {t('w.chatAdmin.2f24db')}
          </Button>
        ) : null}
        <Button loading={busy} disabled={busy || tooLong} onClick={() => onSave([...selected], title.trim())}>
          {t('w.chatAdmin.fadf24')}
        </Button>
      </div>
    </div>
  )
}
