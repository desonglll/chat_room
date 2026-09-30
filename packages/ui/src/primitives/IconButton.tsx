import type { ComponentPropsWithRef, ReactNode } from 'react'
import { cx } from '../internal/cx'
import { Ripple } from './Ripple'
import type { ControlSize } from './types'

/**
 * A square/circular control whose entire content is an icon.
 *
 * `label` is required and is not optional by accident: an icon-only button with no accessible
 * name is the single most common accessibility defect in a chat client, and there are dozens of
 * these in a Telegram window. It becomes `aria-label`, and `title` as well unless the caller
 * overrides it, so a pointer user gets the native tooltip for free. Wrap it in `Tooltip` when a
 * styled tooltip is wanted instead.
 */
export interface IconButtonProps extends Omit<ComponentPropsWithRef<'button'>, 'aria-label'> {
  /** Accessible name. Describes the action, not the glyph: "Attach file", not "paperclip". */
  label: string
  size?: ControlSize | undefined
  variant?: 'plain' | 'filled' | 'tonal' | 'danger' | undefined
  /** Round is Telegram's default for header and composer actions. */
  shape?: 'circle' | 'rounded' | undefined
  /** Sets `aria-pressed`, for a toggle-shaped action such as mute or pin. */
  selected?: boolean | undefined
  ripple?: boolean | undefined
  /** Rendered inside an `aria-hidden` wrapper, because `label` already names the control. */
  children: ReactNode
}

export function IconButton({
  ref,
  label,
  size = 'md',
  variant = 'plain',
  shape = 'circle',
  selected,
  ripple = true,
  className,
  children,
  type = 'button',
  disabled = false,
  title,
  ...rest
}: IconButtonProps) {
  return (
    <button
      {...rest}
      ref={ref}
      type={type}
      disabled={disabled}
      aria-label={label}
      title={title ?? label}
      aria-pressed={selected}
      className={cx(
        'tg-icon-button',
        `tg-icon-button--${variant}`,
        `tg-icon-button--${size}`,
        shape === 'rounded' && 'tg-icon-button--rounded',
        className,
      )}
    >
      {ripple && !disabled ? (
        <Ripple compact tint={variant === 'filled' || variant === 'danger' ? 'inverse' : 'default'} />
      ) : null}
      <span className="tg-icon-button__glyph" aria-hidden="true">
        {children}
      </span>
    </button>
  )
}
