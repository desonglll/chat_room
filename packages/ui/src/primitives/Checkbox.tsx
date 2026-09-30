import { useEffect, useId, useRef, type ReactNode } from 'react'
import { cx } from '../internal/cx'
import { CheckGlyph, DashGlyph } from '../internal/glyphs'
import { useControllable } from '../internal/useControllable'

/**
 * A real `<input type="checkbox">` behind a styled box.
 *
 * The native input is kept (visually hidden, not `display: none`) rather than replaced by a
 * `div role="checkbox"`, because that buys Space activation, form participation, the
 * indeterminate state and label association for free — and every one of those is a thing a
 * hand-rolled checkbox gets wrong.
 *
 * `indeterminate` is set on the DOM node in an effect: it is a property, not an attribute, and
 * there is no React prop for it. It makes the element report `aria-checked="mixed"` natively.
 */
export interface CheckboxProps {
  checked?: boolean | undefined
  defaultChecked?: boolean | undefined
  indeterminate?: boolean | undefined
  onCheckedChange?: ((checked: boolean) => void) | undefined
  label?: ReactNode
  description?: ReactNode
  /** Telegram uses a round check in media selection and a square one in settings. */
  shape?: 'square' | 'round' | undefined
  size?: 'sm' | 'md' | undefined
  disabled?: boolean | undefined
  name?: string | undefined
  value?: string | undefined
  id?: string | undefined
  'aria-label'?: string | undefined
  className?: string | undefined
}

export function Checkbox({
  checked,
  defaultChecked = false,
  indeterminate = false,
  onCheckedChange,
  label,
  description,
  shape = 'square',
  size = 'md',
  disabled = false,
  name,
  value,
  id,
  'aria-label': ariaLabel,
  className,
}: CheckboxProps) {
  const generated = useId()
  const controlId = id ?? `${generated}checkbox`
  const descriptionId = `${generated}description`
  const inputRef = useRef<HTMLInputElement>(null)
  const [selected, setSelected] = useControllable(checked, defaultChecked, onCheckedChange)

  useEffect(() => {
    if (inputRef.current !== null) inputRef.current.indeterminate = indeterminate
  }, [indeterminate])

  const hasText = (label !== undefined && label !== null) || (description !== undefined && description !== null)

  return (
    <span
      className={cx(
        'tg-checkbox',
        `tg-checkbox--${size}`,
        shape === 'round' && 'tg-checkbox--round',
        disabled && 'tg-checkbox--disabled',
        className,
      )}
    >
      <input
        ref={inputRef}
        id={controlId}
        className="tg-checkbox__input"
        type="checkbox"
        name={name}
        value={value}
        checked={selected}
        disabled={disabled}
        aria-label={label === undefined || label === null ? ariaLabel : undefined}
        aria-describedby={description === undefined || description === null ? undefined : descriptionId}
        onChange={(event) => setSelected(event.currentTarget.checked)}
      />
      <span className="tg-checkbox__box" aria-hidden="true">
        {indeterminate ? <DashGlyph className="tg-checkbox__mark" /> : <CheckGlyph className="tg-checkbox__mark" />}
      </span>
      {hasText ? (
        <span className="tg-checkbox__text">
          {label === undefined || label === null ? null : (
            <label className="tg-checkbox__label" htmlFor={controlId}>
              {label}
            </label>
          )}
          {description === undefined || description === null ? null : (
            <span className="tg-checkbox__description" id={descriptionId}>
              {description}
            </span>
          )}
        </span>
      ) : null}
    </span>
  )
}
