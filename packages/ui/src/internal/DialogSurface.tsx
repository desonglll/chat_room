import { useId, useRef, type CSSProperties, type ReactNode, type RefObject } from 'react'
import { cx } from './cx'
import { CrossGlyph } from './glyphs'
import { Portal } from './Portal'
import { useDismiss } from './useDismiss'
import { useFocusTrap } from './useFocusTrap'
import { usePresence } from './usePresence'
import { useRestoreFocus } from './useRestoreFocus'
import { useScrollLock } from './useScrollLock'

/**
 * The shared body of `Modal` and `Sheet`.
 *
 * They are the same widget with different geometry and different entry motion - a centred card
 * that scales in, versus an edge-attached panel that slides in - so the parts that are easy to get
 * wrong (the focus trap, the labelling, the scroll lock, the backdrop, the exit animation) exist
 * once. Keeping it portal-free below the `Portal` wrapper also means its markup and ARIA can be
 * rendered and asserted directly in tests.
 */
export type DialogDismissReason = 'escape' | 'outside-pointer' | 'close-button'

export interface DialogSurfaceProps {
  open: boolean
  onClose: (reason: DialogDismissReason) => void
  /** Drives the class prefix and therefore the geometry and motion. */
  kind: 'modal' | 'sheet'
  /** For a sheet: which edge it is attached to. */
  side?: 'top' | 'right' | 'bottom' | 'left' | undefined
  /** Extra class on the surface, for the size modifier the public component computes. */
  surfaceClassName?: string | undefined
  /** Inline size override, as a CSS length. */
  extent?: string | undefined
  title?: ReactNode
  description?: ReactNode
  footer?: ReactNode
  children?: ReactNode
  closeOnBackdrop?: boolean | undefined
  closeOnEscape?: boolean | undefined
  /** Where focus lands on open. Defaults to the first tab stop inside the surface. */
  initialFocusRef?: RefObject<HTMLElement | null> | undefined
  /** Accessible name when no `title` is rendered. */
  ariaLabel?: string | undefined
  showClose?: boolean | undefined
  /** Accessible name of the close button. */
  closeLabel?: string | undefined
  /** Telegram's bottom-sheet drag handle. Decorative. */
  grabber?: boolean | undefined
  /**
   * Render in place instead of portalling into `document.body`. For a consumer that already owns a
   * full-viewport layer root, and for the markup tests, which have no DOM to portal into.
   */
  inline?: boolean | undefined
  className?: string | undefined
}

export function DialogSurface({
  open,
  onClose,
  kind,
  side = 'bottom',
  surfaceClassName,
  extent,
  title,
  description,
  footer,
  children,
  closeOnBackdrop = true,
  closeOnEscape = true,
  initialFocusRef,
  ariaLabel,
  showClose = true,
  closeLabel = 'Close',
  grabber = false,
  inline = false,
  className,
}: DialogSurfaceProps) {
  const surface = useRef<HTMLDivElement>(null)
  const generated = useId()
  const titleId = `${generated}title`
  const descriptionId = `${generated}description`
  const { present, state } = usePresence(open, surface)

  useScrollLock(present)
  useFocusTrap({ active: open, containerRef: surface, initialFocusRef })
  useRestoreFocus(open)
  useDismiss({
    active: open,
    onDismiss: (reason) => onClose(reason),
    refs: [surface],
    closeOnEscape,
    // Outside-pointer is how a backdrop click is detected: the backdrop is not inside the surface.
    closeOnOutsidePointer: closeOnBackdrop,
  })

  if (!present) return null

  const hasTitle = title !== undefined && title !== null
  const hasDescription = description !== undefined && description !== null

  return (
    <Portal disabled={inline}>
      <div className={cx('tg-dialog', `tg-dialog--${kind}`, kind === 'sheet' && `tg-dialog--${side}`, className)}>
        <div className="tg-dialog__backdrop" data-tg-state={state} />
        <div
          ref={surface}
          role="dialog"
          aria-modal="true"
          aria-labelledby={hasTitle ? titleId : undefined}
          aria-label={hasTitle ? undefined : ariaLabel}
          aria-describedby={hasDescription ? descriptionId : undefined}
          tabIndex={-1}
          data-tg-state={state}
          className={cx('tg-dialog__surface', surfaceClassName)}
          style={extent === undefined ? undefined : ({ '--tg-ui-dialog-extent': extent } as CSSProperties)}
        >
          {grabber ? <span className="tg-dialog__grabber" aria-hidden="true" /> : null}
          {hasTitle || showClose ? (
            <header className="tg-dialog__header">
              {hasTitle ? (
                <h2 className="tg-dialog__title" id={titleId}>
                  {title}
                </h2>
              ) : (
                <span />
              )}
              {showClose ? (
                <button
                  type="button"
                  className="tg-dialog__close"
                  aria-label={closeLabel}
                  onClick={() => onClose('close-button')}
                >
                  <CrossGlyph />
                </button>
              ) : null}
            </header>
          ) : null}
          {hasDescription ? (
            <p className="tg-dialog__description" id={descriptionId}>
              {description}
            </p>
          ) : null}
          {children === undefined || children === null ? null : <div className="tg-dialog__body">{children}</div>}
          {footer === undefined || footer === null ? null : <footer className="tg-dialog__footer">{footer}</footer>}
        </div>
      </div>
    </Portal>
  )
}
