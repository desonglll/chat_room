import type { ReactNode } from 'react'
import { cx } from '../internal/cx'
import { formatCount } from '../internal/badgeText'

/**
 * Counter or status pill. Generic on purpose: it takes a `count`, never an unread-message count —
 * nothing here knows what is being counted.
 *
 * Two accessibility details that are easy to get wrong and are the reason this is a component
 * rather than a CSS class:
 *  - the visible text is truncated ("99+") but `aria-label` carries the exact number, so a screen
 *    reader user is not told the cap;
 *  - `dot` has no text at all, so it MUST be given a `label`, otherwise it is announced as
 *    nothing. Colour alone never carries the meaning (TG-606 contrast ruling, decorative row).
 */
export interface BadgeProps {
  count?: number | undefined
  /** Cap for the visible number. Telegram's is 99. Set 0 to never truncate. */
  max?: number | undefined
  /** Arbitrary content, used instead of `count`. */
  children?: ReactNode
  /** Render as a bare dot with no text. Requires `label`. */
  dot?: boolean | undefined
  variant?: 'accent' | 'muted' | 'danger' | 'success' | undefined
  size?: 'sm' | 'md' | undefined
  /** Keeps the element mounted and animates it out, so the exit easing can play. */
  hidden?: boolean | undefined
  /** Accessible name. Required for `dot`; for a count it defaults to the exact number. */
  label?: string | undefined
  className?: string | undefined
}

export function Badge({
  count,
  max = 99,
  children,
  dot = false,
  variant = 'accent',
  size = 'md',
  hidden = false,
  label,
  className,
}: BadgeProps) {
  const counted = count === undefined ? null : formatCount(count, max)
  const accessibleName = label ?? counted?.exact

  return (
    <span
      className={cx(
        'tg-badge',
        `tg-badge--${variant}`,
        `tg-badge--${size}`,
        dot && 'tg-badge--dot',
        hidden && 'tg-badge--hidden',
        className,
      )}
      aria-hidden={accessibleName === undefined ? true : undefined}
      aria-label={accessibleName}
      role={accessibleName === undefined ? undefined : 'status'}
    >
      {dot ? null : (
        <span className="tg-badge__text" aria-hidden="true">
          {children ?? counted?.text}
        </span>
      )}
    </span>
  )
}
