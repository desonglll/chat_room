import {
  cloneElement,
  useCallback,
  useEffect,
  useId,
  useRef,
  useState,
  type FocusEvent as ReactFocusEvent,
  type HTMLAttributes,
  type PointerEvent as ReactPointerEvent,
  type ReactElement,
  type ReactNode,
  type Ref,
} from 'react'
import type { Placement } from '../internal/positioning'
import { Popover } from './Popover'

type TooltipChild = ReactElement<HTMLAttributes<HTMLElement> & { ref?: Ref<HTMLElement> | undefined }>

/**
 * Hover / focus tooltip.
 *
 * It appears on FOCUS as well as on hover, without a delay, because a tooltip that only responds
 * to a pointer is invisible to a keyboard user - and Telegram's window is full of icon-only
 * controls whose only label is the tooltip. Escape dismisses it while the trigger keeps focus,
 * which is the WAI-ARIA tooltip requirement, and a pointerdown hides it so the tip does not sit
 * over the thing you just clicked.
 *
 * The tip is wired with `aria-describedby`, never `aria-labelledby`: it supplements the control's
 * name rather than replacing it. Use `IconButton`'s `label` for the name.
 *
 * No focus trap and no layer registration: a tooltip must never take Escape away from a dialog
 * underneath it, and it must never take focus.
 */
export interface TooltipProps {
  label: ReactNode
  /** A single element that accepts a ref and DOM event handlers. */
  children: TooltipChild
  placement?: Placement | undefined
  /** Hover dwell before showing. Focus ignores it. */
  openDelay?: number | undefined
  closeDelay?: number | undefined
  disabled?: boolean | undefined
  className?: string | undefined
}

export function Tooltip({
  label,
  children,
  placement = 'top',
  openDelay = 300,
  closeDelay = 80,
  disabled = false,
  className,
}: TooltipProps) {
  const anchor = useRef<HTMLElement>(null)
  const timer = useRef<number | null>(null)
  const [open, setOpen] = useState(false)
  const generated = useId()
  const tipId = `${generated}tooltip`

  const clearTimer = useCallback(() => {
    if (timer.current !== null) window.clearTimeout(timer.current)
    timer.current = null
  }, [])

  const schedule = useCallback(
    (next: boolean, delay: number) => {
      clearTimer()
      if (delay <= 0) {
        setOpen(next)
        return
      }
      timer.current = window.setTimeout(() => setOpen(next), delay)
    },
    [clearTimer],
  )

  useEffect(() => clearTimer, [clearTimer])

  useEffect(() => {
    if (!open) return
    const onKeyDown = (event: KeyboardEvent) => {
      // Not stopped and not prevented: a dialog behind the tooltip must still see its Escape.
      if (event.key === 'Escape') setOpen(false)
    }
    document.addEventListener('keydown', onKeyDown)
    return () => document.removeEventListener('keydown', onKeyDown)
  }, [open])

  const childRef = children.props.ref
  const setAnchor = useCallback(
    (node: HTMLElement | null) => {
      anchor.current = node
      if (typeof childRef === 'function') childRef(node)
      else if (childRef !== null && childRef !== undefined && typeof childRef === 'object') {
        ;(childRef as { current: HTMLElement | null }).current = node
      }
    },
    [childRef],
  )

  const describedBy = [children.props['aria-describedby'], open ? tipId : undefined].filter(Boolean).join(' ')

  const trigger = cloneElement(children, {
    ref: setAnchor,
    'aria-describedby': describedBy === '' ? undefined : describedBy,
    onPointerEnter: (event: ReactPointerEvent<HTMLElement>) => {
      children.props.onPointerEnter?.(event)
      if (!disabled && event.pointerType !== 'touch') schedule(true, openDelay)
    },
    onPointerLeave: (event: ReactPointerEvent<HTMLElement>) => {
      children.props.onPointerLeave?.(event)
      schedule(false, closeDelay)
    },
    onPointerDown: (event: ReactPointerEvent<HTMLElement>) => {
      children.props.onPointerDown?.(event)
      clearTimer()
      setOpen(false)
    },
    onFocus: (event: ReactFocusEvent<HTMLElement>) => {
      children.props.onFocus?.(event)
      if (!disabled) schedule(true, 0)
    },
    onBlur: (event: ReactFocusEvent<HTMLElement>) => {
      children.props.onBlur?.(event)
      clearTimer()
      setOpen(false)
    },
  })

  return (
    <>
      {trigger}
      <Popover
        open={open && !disabled}
        anchor={anchor}
        placement={placement}
        offset={6}
        surface="panel"
        role="tooltip"
        focus="none"
        restoreFocus={false}
        closeOnEscape={false}
        closeOnOutsidePointer={false}
        className={className === undefined ? 'tg-tooltip' : `tg-tooltip ${className}`}
      >
        <span id={tipId} className="tg-tooltip__text">
          {label}
        </span>
      </Popover>
    </>
  )
}
