/**
 * Restrict one member: Telegram's "限制成员" page. The checkboxes start from what the group
 * defaults allow minus the member's current restrictions; switching one off denies it for this
 * member until the chosen moment. A key the group already denies to everyone is not offered.
 */
import { useState } from 'react'
import type { ChatMemberEntry, ChatPermissionsView } from '@tg/core'
import { Button, RadioGroup } from '@tg/ui'
import {
  deniedFrom,
  memberAllowed,
  memberName,
  memberOptions,
  toggleMemberPermission,
  UNTIL_PRESETS,
  untilFromPreset,
  untilText,
} from './chatAdminModel'
import { PermissionChecklist } from './PermissionChecklist'
import { t } from '../../i18n/index'

export interface RestrictionEditorProps {
  target: ChatMemberEntry
  view: ChatPermissionsView
  busy: boolean
  onSave(denied: string[], until: string | null): void
  /** Injected clock for tests. */
  now?: (() => number) | undefined
}

export function RestrictionEditor({ target, view, busy, onSave, now = Date.now }: RestrictionEditorProps) {
  const defaults = view.default_permissions
  const [allowed, setAllowed] = useState<Set<string>>(() => memberAllowed(defaults, target))
  const [preset, setPreset] = useState('day')
  const current = target.restrictions?.[0]
  const options = memberOptions(view.registry, allowed).filter((option) => defaults.includes(option.key))
  const denied = deniedFrom(defaults, allowed)

  return (
    <div className="tg-chatadmin__editor">
      <p className="tg-chatadmin__lead">
        {t('w.chatAdmin.ac90b4')} <strong>{memberName(target)}</strong>
      </p>
      {current ? (
        <p className="tg-chatadmin__note">
          {t('w.chatAdmin.52df91')}
          {untilText(current.until)}
        </p>
      ) : null}
      <PermissionChecklist
        title={t('w.chatAdmin.44983f')}
        options={options}
        disabled={busy}
        onToggle={(key, checked) => setAllowed((set) => toggleMemberPermission(set, key, checked))}
      />
      {denied.length > 0 ? (
        <section className="tg-chatadmin__section">
          <RadioGroup
            name={`restrict-until-${target.user_id}`}
            label={t('w.chatAdmin.c2746f')}
            options={UNTIL_PRESETS.map((entry) => ({ value: entry.id, label: entry.label }))}
            value={preset}
            disabled={busy}
            onValueChange={setPreset}
          />
        </section>
      ) : null}
      <div className="tg-chatadmin__actions">
        <Button
          loading={busy}
          disabled={busy}
          onClick={() => onSave(denied, denied.length > 0 ? untilFromPreset(preset, now()) : null)}
        >
          {denied.length > 0 ? t('w.chatAdmin.ac90b4') : current ? t('w.chatAdmin.ffa17c') : t('w.chatAdmin.fadf24')}
        </Button>
      </div>
    </div>
  )
}
