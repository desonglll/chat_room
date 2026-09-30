import type { ReactNode, RefObject } from 'react'
import { DialogSurface, type DialogDismissReason } from '../internal/DialogSurface'

/**
 * An edge-attached panel: Telegram's bottom sheet on a handheld viewport and its sliding side
 * panels on a wide one.
 *
 * Same dialog semantics as `Modal` - `role="dialog"`, `aria-modal`, focus trap with restoration,
 * layer-aware Escape, scroll lock - and a different geometry and entry animation. It slides in
 * from `side` on `--tg-transition-slide` (Android's EASE_OUT_QUINT, which TG-009 maps to
 * `--tg-ease-decelerate`), and under reduced motion the slide collapses to 1ms so the panel
 * appears in place instead of disappearing.
 *
 * Drag-to-dismiss is deliberately not here: it needs a gesture layer and a spring driven from JS,
 * and it belongs to the feature that owns the sheet rather than to the primitive. The `grabber`
 * affordance is rendered so the shape is right when that lands.
 */
export interface SheetProps {
  open: boolean
  onClose: (reason: DialogDismissReason) => void
  /** Which edge the panel is attached to. Telegram's handheld sheets are `bottom`. */
  side?: 'top' | 'right' | 'bottom' | 'left' | undefined
  title?: ReactNode
  description?: ReactNode
  children?: ReactNode
  footer?: ReactNode
  /** Panel extent along its own axis, as a CSS length. Defaults to the token column width. */
  extent?: string | undefined
  closeOnBackdrop?: boolean | undefined
  closeOnEscape?: boolean | undefined
  showClose?: boolean | undefined
  closeLabel?: string | undefined
  /** Renders the decorative drag handle. On by default for a bottom sheet. */
  grabber?: boolean | undefined
  initialFocusRef?: RefObject<HTMLElement | null> | undefined
  ariaLabel?: string | undefined
  className?: string | undefined
}

export function Sheet({ side = 'bottom', grabber, ...rest }: SheetProps) {
  return <DialogSurface {...rest} kind="sheet" side={side} grabber={grabber ?? side === 'bottom'} />
}
