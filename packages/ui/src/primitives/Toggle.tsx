import { useId, type ReactNode } from 'react'
import { cx } from '../internal/cx'
import { useControllable } from '../internal/useControllable'

/**
 * Telegram's settings switch.
 *
 * A `<button role="switch">` rather than a styled checkbox: `role="switch"` is what makes a screen
 * reader say "on/off" instead of "checked/unchecked", which is the distinction Telegram's settings
 * rows actually carry. Space and Enter both activate it because it is a real button.
 *
 * Controlled when `checked` is passed on first render, uncontrolled otherwise; the mode is pinned
 * for the component's lifetime (see `internal/useControllable.ts`).
 */
export interface ToggleProps {
  checked?: boolean | undefined
  defaultChecked?: boolean | undefined
  onCheckedChange?: ((checked: boolean) => void) | undefined
  /** Row label. Clicking it toggles, because it is wired to the control with `aria-labelledby`. */
  label?: ReactNode
  /** Secondary line under the label, as in Telegram's settings list. */
  description?: ReactNode
  labelPlacement?: 'start' | 'end' | undefined
  size?: 'sm' | 'md' | undefined
  disabled?: boolean | undefined
  /** Accessible name when no visible `label` is rendered. */
  'aria-label'?: string | undefined
  className?: string | undefined
  id?: string | undefined
  name?: string | undefined
}

export function Toggle({
  checked,
  defaultChecked = false,
  onCheckedChange,
  label,
  description,
  labelPlacement = 'start',
  size = 'md',
  disabled = false,
  'aria-label': ariaLabel,
  className,
  id,
  name,
}: ToggleProps) {
  const generated = useId()
  const controlId = id ?? `${generated}toggle`
  const labelId = `${generated}label`
  const descriptionId = `${generated}description`
  const [value, setValue] = useControllable(checked, defaultChecked, onCheckedChange)
  const hasLabel = label !== undefined && label !== null

  const control = (
    <button
      type="button"
      id={controlId}
      name={name}
      role="switch"
      aria-checked={value}
      aria-label={hasLabel ? undefined : ariaLabel}
      aria-labelledby={hasLabel ? labelId : undefined}
      aria-describedby={description === undefined || description === null ? undefined : descriptionId}
      disabled={disabled}
      className="tg-toggle__track"
      onClick={() => setValue(!value)}
    >
      <span className="tg-toggle__thumb" aria-hidden="true" />
    </button>
  )

  if (!hasLabel && (description === undefined || description === null)) {
    return <span className={cx('tg-toggle', `tg-toggle--${size}`, className)}>{control}</span>
  }

  return (
    <div
      className={cx(
        'tg-toggle',
        'tg-toggle--row',
        `tg-toggle--${size}`,
        labelPlacement === 'end' && 'tg-toggle--label-end',
        disabled && 'tg-toggle--disabled',
        className,
      )}
    >
      <span className="tg-toggle__text">
        {hasLabel ? (
          <label className="tg-toggle__label" id={labelId} htmlFor={controlId}>
            {label}
          </label>
        ) : null}
        {description === undefined || description === null ? null : (
          <span className="tg-toggle__description" id={descriptionId}>
            {description}
          </span>
        )}
      </span>
      {control}
    </div>
  )
}
