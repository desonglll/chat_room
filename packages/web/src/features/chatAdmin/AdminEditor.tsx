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
        {existing ? '编辑' : '任命'} <strong>{memberName(target)}</strong> 为管理员
      </p>
      <PermissionChecklist
        title="这位管理员可以"
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
          label="自定义头衔"
          value={title}
          maxLength={MAX_ADMIN_TITLE_CHARS * 2}
          hint="显示在群成员列表中名字旁边"
          error={tooLong ? `最多 ${MAX_ADMIN_TITLE_CHARS} 个字符` : undefined}
          onChange={(event) => setTitle(event.target.value)}
          fullWidth
        />
      </section>
      <div className="tg-chatadmin__actions">
        {existing && onDismiss ? (
          <Button variant="danger" disabled={busy} onClick={onDismiss}>
            撤销管理员
          </Button>
        ) : null}
        <Button loading={busy} disabled={busy || tooLong} onClick={() => onSave([...selected], title.trim())}>
          保存
        </Button>
      </div>
    </div>
  )
}
