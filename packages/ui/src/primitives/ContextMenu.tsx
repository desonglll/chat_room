import {
  useCallback,
  useRef,
  useState,
  type KeyboardEvent as ReactKeyboardEvent,
  type MouseEvent as ReactMouseEvent,
  type PointerEvent as ReactPointerEvent,
  type ReactNode,
} from 'react'
import { cx } from '../internal/cx'
import type { AnchorTarget } from '../internal/useAnchoredPosition'
import { Menu } from './Menu'
import type { MenuItem } from './MenuList'

/**
 * Right-click / long-press menu around an arbitrary region.
 *
 * Three ways in, because Telegram has three and dropping any one of them loses a whole input
 * class:
 *  - `contextmenu` (right click, and the Windows/Linux context-menu key) opens at the pointer;
 *  - a touch press held for `longPressMs` opens at the touch point, and cancels if the finger
 *    moves more than a few pixels so it does not fight a scroll gesture;
 *  - Shift+F10 opens anchored to the region itself, which is the keyboard route and the one that
 *    is almost always missing from a hand-rolled context menu.
 *
 * The wrapper is not focusable by default: it normally contains something focusable already (a
 * chat row, a message bubble) and adding a tab stop around it would double every stop in a list.
 * Set `focusable` when the region has no focusable content of its own.
 */
export interface ContextMenuProps {
  items: readonly MenuItem[]
  children: ReactNode
  disabled?: boolean | undefined
  /** Touch hold duration before the menu opens. Telegram's is 500ms. */
  longPressMs?: number | undefined
  /** Movement in pixels that cancels a hold, so a scroll does not open a menu. */
  longPressSlop?: number | undefined
  onOpenChange?: ((open: boolean) => void) | undefined
  /** Gives the wrapper a tab stop, for a region with no focusable content. */
  focusable?: boolean | undefined
  'aria-label'?: string | undefined
  className?: string | undefined
  /** Passed to the menu: content above the items, given the close function. */
  header?: ((close: () => void) => ReactNode) | undefined
}

export function ContextMenu({
  items,
  children,
  disabled = false,
  longPressMs = 500,
  longPressSlop = 8,
  onOpenChange,
  focusable = false,
  'aria-label': ariaLabel = 'Context menu',
  className,
  header,
}: ContextMenuProps) {
  const region = useRef<HTMLSpanElement>(null)
  const holdTimer = useRef<number | null>(null)
  const holdOrigin = useRef<{ x: number; y: number } | null>(null)
  const [anchor, setAnchor] = useState<AnchorTarget>(null)
  // TG-108: true while a touch hold is being timed, so the content can shrink under the finger.
  const [holding, setHolding] = useState(false)
  // TG-1201: only a touch-opened menu keeps text selection off; a right click must leave the
  // user's selection intact (the message menu's «引用» quotes it).
  const [touchOpen, setTouchOpen] = useState(false)

  const open = anchor !== null

  const show = useCallback(
    (next: AnchorTarget, byTouch = false) => {
      setAnchor(next)
      setTouchOpen(byTouch)
      onOpenChange?.(true)
    },
    [onOpenChange],
  )

  const hide = useCallback(() => {
    setAnchor(null)
    setTouchOpen(false)
    onOpenChange?.(false)
  }, [onOpenChange])

  const cancelHold = useCallback(() => {
    if (holdTimer.current !== null) window.clearTimeout(holdTimer.current)
    holdTimer.current = null
    holdOrigin.current = null
    setHolding(false)
  }, [])

  const onContextMenu = (event: ReactMouseEvent<HTMLSpanElement>) => {
    if (disabled || items.length === 0) return
    event.preventDefault()
    show({ x: event.clientX, y: event.clientY })
  }

  const onPointerDown = (event: ReactPointerEvent<HTMLSpanElement>) => {
    if (disabled || items.length === 0 || event.pointerType !== 'touch') return
    const point = { x: event.clientX, y: event.clientY }
    holdOrigin.current = point
    setHolding(true)
    holdTimer.current = window.setTimeout(() => {
      holdTimer.current = null
      setHolding(false)
      show(point, true)
    }, longPressMs)
  }

  const onPointerMove = (event: ReactPointerEvent<HTMLSpanElement>) => {
    const origin = holdOrigin.current
    if (origin === null) return
    const moved = Math.abs(event.clientX - origin.x) + Math.abs(event.clientY - origin.y)
    if (moved > longPressSlop) cancelHold()
  }

  const onKeyDown = (event: ReactKeyboardEvent<HTMLSpanElement>) => {
    if (disabled || items.length === 0 || open) return
    // Shift+F10 is the platform shortcut; ContextMenu is the dedicated key where it exists.
    const wanted = (event.shiftKey && event.key === 'F10') || event.key === 'ContextMenu'
    if (!wanted) return
    event.preventDefault()
    show(region)
  }

  return (
    <span
      ref={region}
      className={cx('tg-context-menu', className)}
      tabIndex={focusable ? 0 : undefined}
      data-tg-context-open={open ? '' : undefined}
      data-tg-holding={holding ? '' : undefined}
      data-tg-touch-open={open && touchOpen ? '' : undefined}
      onContextMenu={onContextMenu}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={cancelHold}
      onPointerCancel={cancelHold}
      onKeyDown={onKeyDown}
    >
      {children}
      <Menu
        open={open}
        onClose={hide}
        anchor={anchor}
        items={items}
        placement="bottom-start"
        aria-label={ariaLabel}
        triggerRef={region}
        header={header}
      />
    </span>
  )
}
