import type { ReactNode, RefObject } from 'react'
import { DialogSurface, type DialogDismissReason } from '../internal/DialogSurface'

export type { DialogDismissReason }

/**
 * A centred modal dialog.
 *
 * Focus and keyboard, all inherited from `internal/DialogSurface`:
 *  - focus moves into the dialog on open (`initialFocusRef`, else the first tab stop, else the
 *    dialog itself) and returns to whatever had it before, on close;
 *  - Tab and Shift+Tab cycle inside the dialog and cannot reach the page behind it;
 *  - a `focusin` outside the dialog pulls focus back, so programmatic focus cannot escape either;
 *  - Escape closes the topmost layer only, so a `Menu` opened inside takes the first Escape;
 *  - the document is scroll-locked while it is open, with scrollbar-width compensation.
 *
 * Either pass a `title` (which becomes `aria-labelledby`) or an `ariaLabel`. A dialog with no
 * accessible name is announced as "dialog" and nothing more.
 */
export interface ModalProps {
  open: boolean
  onClose: (reason: DialogDismissReason) => void
  title?: ReactNode
  /** Secondary line under the title, wired to `aria-describedby`. */
  description?: ReactNode
  children?: ReactNode
  /** Action row, pinned below the body. */
  footer?: ReactNode
  size?: 'sm' | 'md' | 'lg' | undefined
  closeOnBackdrop?: boolean | undefined
  closeOnEscape?: boolean | undefined
  showClose?: boolean | undefined
  closeLabel?: string | undefined
  initialFocusRef?: RefObject<HTMLElement | null> | undefined
  /** Accessible name when no visible `title` is rendered. */
  ariaLabel?: string | undefined
  className?: string | undefined
}

export function Modal({ size = 'md', className, ...rest }: ModalProps) {
  return <DialogSurface {...rest} kind="modal" surfaceClassName={`tg-dialog__surface--${size}`} className={className} />
}
