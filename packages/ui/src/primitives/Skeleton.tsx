import type { CSSProperties } from 'react'
import { cx } from '../internal/cx'

/**
 * Loading placeholder. Decorative by definition: it is `aria-hidden`, and the region it fills is
 * expected to carry `aria-busy` so assistive technology hears "busy" instead of a wall of nothing.
 *
 * `lines > 1` with `variant="text"` renders a paragraph shape whose last line is short, because a
 * block of equal-length bars does not read as text.
 */
export interface SkeletonProps {
  variant?: 'text' | 'rect' | 'circle' | undefined
  width?: number | string | undefined
  height?: number | string | undefined
  /** Only meaningful for `variant="text"`. */
  lines?: number | undefined
  /** CSS length or token reference for a `rect`'s corner radius. */
  radius?: string | undefined
  animated?: boolean | undefined
  className?: string | undefined
}

export function Skeleton({
  variant = 'text',
  width,
  height,
  lines = 1,
  radius,
  animated = true,
  className,
}: SkeletonProps) {
  const style: CSSProperties = {}
  if (width !== undefined) style.inlineSize = width
  if (height !== undefined) style.blockSize = height
  if (radius !== undefined) style.borderRadius = radius

  const classes = cx('tg-skeleton', `tg-skeleton--${variant}`, animated && 'tg-skeleton--animated', className)

  if (variant === 'text' && lines > 1) {
    return (
      <span
        className="tg-skeleton-stack"
        aria-hidden="true"
        style={width === undefined ? undefined : { inlineSize: width }}
      >
        {Array.from({ length: lines }, (_, index) => (
          <span key={index} className={cx(classes, index === lines - 1 && 'tg-skeleton--short')} />
        ))}
      </span>
    )
  }

  return <span className={classes} aria-hidden="true" style={style} />
}
