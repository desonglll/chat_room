/** A column of permission checkboxes (Telegram's settings-style square checks). */
import { Checkbox } from '@tg/ui'
import type { PermissionOption } from './chatAdminModel'

export interface PermissionChecklistProps {
  title: string
  options: readonly PermissionOption[]
  disabled?: boolean | undefined
  onToggle(key: string, checked: boolean): void
}

export function PermissionChecklist({ title, options, disabled = false, onToggle }: PermissionChecklistProps) {
  return (
    <fieldset className="tg-chatadmin__checklist">
      <legend className="tg-chatadmin__section-title">{title}</legend>
      {options.map((option) => (
        <Checkbox
          key={option.key}
          className="tg-chatadmin__check"
          label={option.label}
          name={option.key}
          value={option.key}
          checked={option.checked}
          disabled={disabled || option.disabled}
          onCheckedChange={(checked) => onToggle(option.key, checked)}
        />
      ))}
    </fieldset>
  )
}
