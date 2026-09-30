import { useState } from 'react'
import type { ReactNode } from 'react'
import { cx } from '../internal/cx'
import { initialsFrom, paletteIndex } from '../internal/initials'
import type { ControlSize } from './types'

/** Number of palette slots. Telegram has seven peer colours. */
export const AVATAR_PALETTE_SLOTS = 7

/**
 * Circular (or forum-rounded) image with an initials fallback.
 *
 * Generic: it takes `src` and `label`, never a user or a chat. `label` is both the accessible
 * name and the source of the initials and of the deterministic palette slot, so the caller does
 * not have to compute any of the three.
 *
 * The fallback palette needs seven colours and TG-009's 197 semantic tokens do not include them,
 * so `--tg-avatar-<n>` is read with `--tg-accent` as the `var()` fallback: correct today, seven
 * distinct colours as soon as the integration patch in docs/devlog/TG-010.md lands. Until then all
 * seven slots render as the accent, which is a visible limitation and not a silent one.
 */
export interface AvatarProps {
  src?: string | undefined
  /** Accessible name, initials source, and palette seed. */
  label: string
  /** Override the computed initials (for example a single emoji). */
  initials?: string | undefined
  size?: ControlSize | 'xl' | number | undefined
  /** `forum` is Telegram's squarish topic avatar (37% radius). */
  shape?: 'circle' | 'forum' | undefined
  /** Pin the palette slot instead of deriving it from `label`. */
  colorIndex?: number | undefined
  /** Renders the online dot. Never the only signal: it also sets `data-tg-online`. */
  online?: boolean | undefined
  /** Corner slot, typically a `Badge`. */
  badge?: ReactNode
  className?: string | undefined
}

export function Avatar({
  src,
  label,
  initials,
  size = 'md',
  shape = 'circle',
  colorIndex,
  online = false,
  badge,
  className,
}: AvatarProps) {
  const [broken, setBroken] = useState(false)
  const named = typeof size === 'string'
  const slot = (colorIndex ?? paletteIndex(label, AVATAR_PALETTE_SLOTS)) % AVATAR_PALETTE_SLOTS
  const showImage = src !== undefined && src !== '' && !broken
  const text = initials ?? initialsFrom(label)

  return (
    <span
      className={cx(
        'tg-avatar',
        named && `tg-avatar--${size}`,
        shape === 'forum' && 'tg-avatar--forum',
        `tg-avatar--slot-${slot}`,
        className,
      )}
      style={named ? undefined : { inlineSize: size, blockSize: size }}
      data-tg-online={online ? '' : undefined}
    >
      {showImage ? (
        <img className="tg-avatar__image" src={src} alt={label} onError={() => setBroken(true)} draggable={false} />
      ) : (
        <span className="tg-avatar__initials" role="img" aria-label={label}>
          {text}
        </span>
      )}
      {online ? <span className="tg-avatar__online" aria-hidden="true" /> : null}
      {badge === undefined || badge === null ? null : <span className="tg-avatar__badge">{badge}</span>}
    </span>
  )
}
