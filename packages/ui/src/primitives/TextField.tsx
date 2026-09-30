import { useId, type ComponentPropsWithoutRef, type ReactNode, type Ref } from 'react'
import { cx } from '../internal/cx'
import { CrossGlyph } from '../internal/glyphs'
import type { ControlSize } from './types'

/**
 * Telegram's text input: a rounded bordered box with a label that floats up into the border when
 * the field is focused or filled.
 *
 * The float is pure CSS (`:focus-within` and `:placeholder-shown`), which is why the component
 * forces `placeholder=" "` whenever a `label` is given — the selector needs a placeholder to test
 * emptiness, and Telegram never shows a label and a placeholder at the same time. A `placeholder`
 * passed alongside a `label` is therefore ignored, and that is deliberate rather than a bug.
 *
 * `hint` and `error` are wired through `aria-describedby`, and `error` also sets `aria-invalid`
 * and is announced (`role="alert"`), because a field that only turns red communicates nothing to
 * a screen reader and nothing to a colour-blind user.
 */
export interface TextFieldProps extends Omit<ComponentPropsWithoutRef<'input'>, 'size'> {
  ref?: Ref<HTMLInputElement | HTMLTextAreaElement> | undefined
  label?: string | undefined
  /** Helper text below the field. Hidden while `error` is set. */
  hint?: ReactNode
  /** Message shown instead of `hint`. Its presence is what makes the field invalid. */
  error?: string | undefined
  size?: ControlSize | undefined
  multiline?: boolean | undefined
  rows?: number | undefined
  startAdornment?: ReactNode
  endAdornment?: ReactNode
  /** Shows a clear button while the field has a value. */
  clearable?: boolean | undefined
  onClear?: (() => void) | undefined
  /** Accessible name for the clear button. */
  clearLabel?: string | undefined
  fullWidth?: boolean | undefined
}

export function TextField({
  ref,
  label,
  hint,
  error,
  size = 'md',
  multiline = false,
  rows = 3,
  startAdornment,
  endAdornment,
  clearable = false,
  onClear,
  clearLabel = 'Clear',
  fullWidth = false,
  className,
  id,
  placeholder,
  disabled = false,
  required = false,
  value,
  ...rest
}: TextFieldProps) {
  const generated = useId()
  const fieldId = id ?? `${generated}field`
  const hintId = `${generated}hint`
  const invalid = error !== undefined && error !== ''
  const described = invalid || (hint !== undefined && hint !== null) ? hintId : undefined
  const hasValue = value !== undefined && value !== null && String(value) !== ''

  const shared = {
    ...rest,
    id: fieldId,
    disabled,
    required,
    value,
    className: 'tg-field__control',
    placeholder: label === undefined ? placeholder : ' ',
    'aria-invalid': invalid || undefined,
    'aria-describedby': described,
  }

  return (
    <div
      className={cx(
        'tg-field',
        `tg-field--${size}`,
        multiline && 'tg-field--multiline',
        fullWidth && 'tg-field--block',
        invalid && 'tg-field--invalid',
        disabled && 'tg-field--disabled',
        className,
      )}
    >
      <div className="tg-field__box">
        {startAdornment === undefined || startAdornment === null ? null : (
          <span className="tg-field__adornment" aria-hidden="true">
            {startAdornment}
          </span>
        )}
        {multiline ? (
          // One cast, in one place: `rest` is typed as input attributes and a textarea accepts the
          // same set minus `type`, which this component does not forward for the multiline case.
          <textarea
            {...(shared as unknown as ComponentPropsWithoutRef<'textarea'>)}
            ref={ref as Ref<HTMLTextAreaElement>}
            rows={rows}
          />
        ) : (
          <input {...shared} ref={ref as Ref<HTMLInputElement>} />
        )}
        {label === undefined ? null : (
          <label className="tg-field__label" htmlFor={fieldId}>
            {label}
            {required ? <span aria-hidden="true">{' *'}</span> : null}
          </label>
        )}
        {clearable && hasValue && !disabled ? (
          <button type="button" className="tg-field__clear" aria-label={clearLabel} onClick={onClear}>
            <CrossGlyph />
          </button>
        ) : null}
        {endAdornment === undefined || endAdornment === null ? null : (
          <span className="tg-field__adornment" aria-hidden="true">
            {endAdornment}
          </span>
        )}
      </div>
      {invalid ? (
        <p className="tg-field__message tg-field__message--error" id={hintId} role="alert">
          {error}
        </p>
      ) : hint === undefined || hint === null ? null : (
        <p className="tg-field__message" id={hintId}>
          {hint}
        </p>
      )}
    </div>
  )
}
