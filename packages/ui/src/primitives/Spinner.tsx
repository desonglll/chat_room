import { cx } from '../internal/cx'
import type { ControlSize, Tint } from './types'

/**
 * Indeterminate or determinate progress ring.
 *
 * Default is decorative: no `label` means `aria-hidden`, which is correct when the spinner sits
 * inside something that already announces itself as busy (`Button`'s `loading`). Passing `label`
 * promotes it to a live `role="status"` for the standalone case.
 *
 * Reduced motion: a spinner is the one animation that must NOT collapse — a frozen spinner says
 * "hung", and TG-009's movement durations go to 1ms, which would strobe. The rotation therefore
 * runs off a dedicated loop period; see `Spinner.css`.
 */
export interface SpinnerProps {
  size?: ControlSize | number | undefined
  tint?: Tint | undefined
  /** Accessible name. Omit for a decorative spinner inside an already-busy control. */
  label?: string | undefined
  /** 0..1 for a determinate arc. Omit for the indeterminate sweep. */
  progress?: number | undefined
  className?: string | undefined
}

const RADIUS = 10
const CIRCUMFERENCE = 2 * Math.PI * RADIUS

export function Spinner({ size = 'md', tint = 'accent', label, progress, className }: SpinnerProps) {
  const determinate = progress !== undefined
  const clamped = determinate ? Math.min(1, Math.max(0, progress)) : 0
  const named = typeof size === 'string'
  const percent = Math.round(clamped * 100)

  return (
    <span
      className={cx(
        'tg-spinner',
        named && `tg-spinner--${size}`,
        `tg-spinner--${tint}`,
        determinate && 'tg-spinner--determinate',
        className,
      )}
      style={named ? undefined : { inlineSize: size, blockSize: size }}
      role={label === undefined ? undefined : determinate ? 'progressbar' : 'status'}
      aria-label={label}
      aria-hidden={label === undefined ? true : undefined}
      aria-valuenow={label !== undefined && determinate ? percent : undefined}
      aria-valuemin={label !== undefined && determinate ? 0 : undefined}
      aria-valuemax={label !== undefined && determinate ? 100 : undefined}
    >
      <svg className="tg-spinner__track" viewBox="0 0 24 24" focusable="false" aria-hidden="true">
        <circle
          className="tg-spinner__arc"
          cx="12"
          cy="12"
          r={RADIUS}
          strokeDasharray={determinate ? CIRCUMFERENCE : undefined}
          strokeDashoffset={determinate ? CIRCUMFERENCE * (1 - clamped) : undefined}
        />
      </svg>
    </span>
  )
}
