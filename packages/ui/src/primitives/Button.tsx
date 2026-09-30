import type { ComponentPropsWithRef, ReactNode } from 'react'
import { cx } from '../internal/cx'
import { Ripple } from './Ripple'
import { Spinner } from './Spinner'
import type { ControlSize, ControlVariant } from './types'

/**
 * Telegram's text button. Four treatments, three sizes, two shapes.
 *
 * `type` defaults to `button`: a primitive that silently submits the nearest form is a bug that
 * only shows up once someone puts it in a form.
 *
 * Contrast: `variant="filled"` paints `--tg-text-on-accent` on `--tg-accent`, which measures
 * 3.31:1 on the day theme. That is below the 4.5:1 body-text requirement and is kept on purpose
 * — see the contrast ruling on the TG-606 card: text on accent fills keeps Telegram's values.
 */
export interface ButtonProps extends ComponentPropsWithRef<'button'> {
  variant?: ControlVariant | undefined
  size?: ControlSize | undefined
  /** `pill` is Telegram's fully rounded action button; `control` is the 16px field radius. */
  shape?: 'control' | 'pill' | undefined
  fullWidth?: boolean | undefined
  /**
   * Swaps the label for a `Spinner` and sets `aria-busy`. The button stays in the tab order and
   * keeps its accessible name, so a screen reader announces the same control as busy rather than
   * having it disappear.
   */
  loading?: boolean | undefined
  /** Set false for a button inside a dense list where 700ms of wave is noise. */
  ripple?: boolean | undefined
  startIcon?: ReactNode
  endIcon?: ReactNode
}

export function Button({
  ref,
  variant = 'filled',
  size = 'md',
  shape = 'control',
  fullWidth = false,
  loading = false,
  ripple = true,
  startIcon,
  endIcon,
  className,
  children,
  type = 'button',
  disabled = false,
  ...rest
}: ButtonProps) {
  const inert = disabled || loading

  return (
    <button
      {...rest}
      ref={ref}
      type={type}
      disabled={disabled}
      aria-busy={loading || undefined}
      data-tg-loading={loading ? '' : undefined}
      className={cx(
        'tg-button',
        `tg-button--${variant}`,
        `tg-button--${size}`,
        shape === 'pill' && 'tg-button--pill',
        fullWidth && 'tg-button--block',
        className,
      )}
    >
      {ripple && !inert ? <Ripple tint={variant === 'filled' || variant === 'danger' ? 'inverse' : 'accent'} /> : null}
      {loading ? (
        <span className="tg-button__loader">
          <Spinner size="sm" tint={variant === 'filled' || variant === 'danger' ? 'inverse' : 'accent'} />
        </span>
      ) : null}
      {startIcon !== undefined && startIcon !== null ? (
        <span className="tg-button__icon" aria-hidden="true">
          {startIcon}
        </span>
      ) : null}
      <span className="tg-button__label">{children}</span>
      {endIcon !== undefined && endIcon !== null ? (
        <span className="tg-button__icon" aria-hidden="true">
          {endIcon}
        </span>
      ) : null}
    </button>
  )
}
