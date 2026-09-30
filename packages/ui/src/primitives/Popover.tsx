import { useRef, type ReactNode, type RefObject } from 'react'
import { cx } from '../internal/cx'
import { Portal } from '../internal/Portal'
import { useAnchoredPosition, type AnchorTarget } from '../internal/useAnchoredPosition'
import { useDismiss } from '../internal/useDismiss'
import { useFocusTrap } from '../internal/useFocusTrap'
import { usePresence } from '../internal/usePresence'
import { useRestoreFocus } from '../internal/useRestoreFocus'
import type { Placement } from '../internal/positioning'

export type DismissReason = 'escape' | 'outside-pointer' | 'select' | 'backdrop'

/**
 * The anchored floating layer that `Menu`, `ContextMenu` and `Tooltip` are built on, and a
 * component in its own right for arbitrary panels (emoji picker, reaction bar, date picker).
 *
 * What it owns, and why each piece is here rather than in the consumer:
 *  - placement, with flipping and viewport clamping (`internal/positioning.ts`, pure and tested);
 *  - a portal, so the panel is never clipped by an ancestor's `overflow`;
 *  - focus handling, `trap` by default, with restoration. The three modes exist because the three
 *    consumers genuinely differ: a panel traps Tab, a `Menu` only MOVES focus in (the ARIA menu
 *    pattern makes Tab dismiss it), and a `Tooltip` must never touch focus at all - taking focus
 *    would blur its own trigger and close it;
 *  - Escape and outside-pointer dismissal, layer aware so only the topmost panel closes;
 *  - exit animation, via `usePresence`.
 *
 * `role` is deliberately not defaulted to `dialog`: a popover with no name and `role="dialog"` is
 * worse for a screen reader than a plain group. Set it explicitly together with a name.
 */
export interface PopoverProps {
  open: boolean
  onClose?: ((reason: DismissReason) => void) | undefined
  /** An element ref to attach to, or a viewport point (`{ x, y }`) to open at. */
  anchor: AnchorTarget
  placement?: Placement | undefined
  /** Gap between anchor and panel, in pixels. */
  offset?: number | undefined
  /** Minimum distance kept from the viewport edge, in pixels. */
  viewportPadding?: number | undefined
  matchAnchorWidth?: boolean | undefined
  /**
   * `trap` confines Tab to the panel; `move` puts focus on the first control inside but leaves Tab
   * alone; `none` never touches focus.
   */
  focus?: 'trap' | 'move' | 'none' | undefined
  restoreFocus?: boolean | undefined
  closeOnEscape?: boolean | undefined
  closeOnOutsidePointer?: boolean | undefined
  /** `menu` is the blurred floating menu treatment; `panel` is the opaque card. */
  surface?: 'menu' | 'panel' | undefined
  role?: string | undefined
  'aria-label'?: string | undefined
  'aria-labelledby'?: string | undefined
  initialFocusRef?: RefObject<HTMLElement | null> | undefined
  /** Extra elements that count as "inside" for outside-pointer dismissal, typically the trigger. */
  insideRefs?: readonly RefObject<HTMLElement | null>[] | undefined
  className?: string | undefined
  children: ReactNode
}

export function Popover({
  open,
  onClose,
  anchor,
  placement = 'bottom-start',
  offset = 8,
  viewportPadding = 8,
  matchAnchorWidth = false,
  focus = 'trap',
  restoreFocus = true,
  closeOnEscape = true,
  closeOnOutsidePointer = true,
  surface = 'panel',
  role,
  'aria-label': ariaLabel,
  'aria-labelledby': ariaLabelledBy,
  initialFocusRef,
  insideRefs,
  className,
  children,
}: PopoverProps) {
  const panel = useRef<HTMLDivElement>(null)
  const { present, state } = usePresence(open, panel)
  const position = useAnchoredPosition({
    active: present,
    anchor,
    floatingRef: panel,
    placement,
    offset,
    padding: viewportPadding,
  })

  useFocusTrap({ active: open && focus !== 'none', containerRef: panel, initialFocusRef, trap: focus === 'trap' })
  useRestoreFocus(open, restoreFocus)
  useDismiss({
    active: open,
    onDismiss: (reason) => onClose?.(reason),
    refs: [panel, ...(insideRefs ?? [])],
    closeOnEscape,
    closeOnOutsidePointer,
  })

  if (!present) return null

  return (
    <Portal>
      <div
        ref={panel}
        role={role}
        aria-label={ariaLabel}
        aria-labelledby={ariaLabelledBy}
        tabIndex={-1}
        data-tg-state={state}
        data-tg-placement={position.placement}
        className={cx('tg-popover', `tg-popover--${surface}`, className)}
        style={{
          // Measurement happens in a layout effect, which React flushes - together with the
          // re-render it triggers - before the browser paints. So the panel is never seen at 0,0
          // and needs no `visibility: hidden` gate. That matters: a visibility-hidden element
          // cannot take focus, which would break the focus move on open.
          left: position.x,
          top: position.y,
          inlineSize: matchAnchorWidth && position.measured ? position.anchorWidth : undefined,
        }}
      >
        {children}
      </div>
    </Portal>
  )
}
