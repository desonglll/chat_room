import { useId, useRef, type KeyboardEvent as ReactKeyboardEvent, type ReactNode } from 'react'
import { cx } from '../internal/cx'
import { nextFocusIndex } from '../internal/rovingFocus'
import { useControllable } from '../internal/useControllable'

/**
 * `Radio` and `RadioGroup`.
 *
 * Native `<input type="radio">` elements sharing a `name` already give arrow-key navigation, the
 * roving tab stop and "one stop for the whole group" — reimplementing that on top of
 * `role="radio"` divs is how a hand-rolled radio ends up worse than the platform. `RadioGroup`
 * therefore adds only what the platform is missing: a `role="radiogroup"` container with an
 * accessible name, and Home / End, which browsers do not implement for radios.
 */
export interface RadioProps {
  value: string
  label?: ReactNode
  description?: ReactNode
  disabled?: boolean | undefined
  /** Set by `RadioGroup`; only needed when a `Radio` is used outside one. */
  name?: string | undefined
  checked?: boolean | undefined
  onSelect?: ((value: string) => void) | undefined
  size?: 'sm' | 'md' | undefined
  id?: string | undefined
  className?: string | undefined
}

export function Radio({
  value,
  label,
  description,
  disabled = false,
  name,
  checked,
  onSelect,
  size = 'md',
  id,
  className,
}: RadioProps) {
  const generated = useId()
  const controlId = id ?? `${generated}radio`
  const descriptionId = `${generated}description`

  return (
    <span
      className={cx('tg-radio', `tg-radio--${size}`, disabled && 'tg-radio--disabled', className)}
      data-tg-radio-value={value}
    >
      <input
        id={controlId}
        className="tg-radio__input"
        type="radio"
        name={name}
        value={value}
        checked={checked}
        disabled={disabled}
        aria-describedby={description === undefined || description === null ? undefined : descriptionId}
        onChange={() => onSelect?.(value)}
      />
      <span className="tg-radio__dot" aria-hidden="true" />
      {(label === undefined || label === null) && (description === undefined || description === null) ? null : (
        <span className="tg-radio__text">
          {label === undefined || label === null ? null : (
            <label className="tg-radio__label" htmlFor={controlId}>
              {label}
            </label>
          )}
          {description === undefined || description === null ? null : (
            <span className="tg-radio__description" id={descriptionId}>
              {description}
            </span>
          )}
        </span>
      )}
    </span>
  )
}

export interface RadioOption {
  value: string
  label?: ReactNode
  description?: ReactNode
  disabled?: boolean | undefined
}

export interface RadioGroupProps {
  /** Shared `name`. Required: it is what makes the browser treat the inputs as one group. */
  name: string
  options: readonly RadioOption[]
  value?: string | undefined
  defaultValue?: string | undefined
  onValueChange?: ((value: string) => void) | undefined
  /** Visible group label. Rendered and referenced by `aria-labelledby`. */
  label?: ReactNode
  /** Accessible name when no visible `label` is rendered. */
  'aria-label'?: string | undefined
  orientation?: 'vertical' | 'horizontal' | undefined
  size?: 'sm' | 'md' | undefined
  disabled?: boolean | undefined
  className?: string | undefined
}

export function RadioGroup({
  name,
  options,
  value,
  defaultValue = '',
  onValueChange,
  label,
  'aria-label': ariaLabel,
  orientation = 'vertical',
  size = 'md',
  disabled = false,
  className,
}: RadioGroupProps) {
  const generated = useId()
  const labelId = `${generated}label`
  const container = useRef<HTMLDivElement>(null)
  const [selected, setSelected] = useControllable(value, defaultValue, onValueChange)
  const hasLabel = label !== undefined && label !== null

  // Home / End only. Arrow keys are the browser's job here and doing both would fight it.
  const onKeyDown = (event: ReactKeyboardEvent<HTMLDivElement>) => {
    if (event.key !== 'Home' && event.key !== 'End') return
    const result = nextFocusIndex({
      key: event.key,
      current: options.findIndex((option) => option.value === selected),
      count: options.length,
      orientation,
      isDisabled: (index) => disabled || options[index]?.disabled === true,
    })
    if (!result.handled) return
    const target = options[result.index]
    if (target === undefined) return
    event.preventDefault()
    setSelected(target.value)
    container.current?.querySelector<HTMLInputElement>(`input[value="${CSS.escape(target.value)}"]`)?.focus()
  }

  return (
    <div
      ref={container}
      role="radiogroup"
      aria-labelledby={hasLabel ? labelId : undefined}
      aria-label={hasLabel ? undefined : ariaLabel}
      aria-orientation={orientation}
      onKeyDown={onKeyDown}
      className={cx('tg-radio-group', `tg-radio-group--${orientation}`, className)}
    >
      {hasLabel ? (
        <span className="tg-radio-group__label" id={labelId}>
          {label}
        </span>
      ) : null}
      <div className="tg-radio-group__options">
        {options.map((option) => (
          <Radio
            key={option.value}
            value={option.value}
            label={option.label}
            description={option.description}
            disabled={disabled || option.disabled === true}
            name={name}
            size={size}
            checked={selected === option.value}
            onSelect={setSelected}
          />
        ))}
      </div>
    </div>
  )
}
