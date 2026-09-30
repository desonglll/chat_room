import {
  useCallback,
  useRef,
  useState,
  type ComponentPropsWithoutRef,
  type Ref,
  type UIEvent as ReactUIEvent,
} from 'react'
import { cx } from '../internal/cx'

/**
 * A scroll container with Telegram's thin overlay scrollbar.
 *
 * It scrolls natively. That is the whole point: a JS-driven custom scrollbar breaks momentum
 * scrolling, breaks `scrollIntoView`, breaks the browser's find-in-page, and would fight
 * `react-virtuoso` in TG-101 - which is the component that will actually carry the message list.
 * Styling is `scrollbar-width`/`scrollbar-color` with a `::-webkit-scrollbar` fallback, both fed
 * from the `--tg-scrollbar` tokens.
 *
 * `overlay` reproduces tweb's behaviour where the bar is invisible until you scroll or hover.
 *
 * `focusable` is off by default and that is a real trade-off, recorded in the devlog: a scroll
 * region SHOULD be keyboard focusable when it has no focusable content, but a chat list or a
 * message list is full of focusable children and adding a stop around each one doubles the tab
 * stops in the window. Set it for a region of plain text.
 */
export interface ScrollAreaProps extends Omit<ComponentPropsWithoutRef<'div'>, 'ref'> {
  /** Ref to the scrolling element itself, for `scrollTo` and for virtualiser integration. */
  viewportRef?: Ref<HTMLDivElement> | undefined
  orientation?: 'vertical' | 'horizontal' | 'both' | undefined
  /** Fade the bar out when idle, as tweb does. */
  overlay?: boolean | undefined
  /** Mask the scrolled-away edges, so content fades rather than being cut off. */
  fadeEdges?: boolean | undefined
  /** Adds a tab stop so a region with no focusable content can be scrolled by keyboard. */
  focusable?: boolean | undefined
  /** Accessible name. Only meaningful together with `focusable`. */
  'aria-label'?: string | undefined
}

export function ScrollArea({
  viewportRef,
  orientation = 'vertical',
  overlay = true,
  fadeEdges = false,
  focusable = false,
  className,
  children,
  onScroll,
  ...rest
}: ScrollAreaProps) {
  const idleTimer = useRef<number | null>(null)
  const [scrolling, setScrolling] = useState(false)

  const handleScroll = useCallback(
    (event: ReactUIEvent<HTMLDivElement>) => {
      onScroll?.(event)
      if (!overlay) return
      setScrolling(true)
      if (idleTimer.current !== null) window.clearTimeout(idleTimer.current)
      // Long enough that the bar does not blink between two flicks of a trackpad.
      idleTimer.current = window.setTimeout(() => setScrolling(false), 700)
    },
    [onScroll, overlay],
  )

  return (
    <div
      {...rest}
      ref={viewportRef}
      className={cx(
        'tg-scroll',
        `tg-scroll--${orientation}`,
        overlay && 'tg-scroll--overlay',
        fadeEdges && 'tg-scroll--fade',
        className,
      )}
      tabIndex={focusable ? 0 : undefined}
      role={focusable ? 'group' : undefined}
      data-tg-scrolling={scrolling ? '' : undefined}
      onScroll={handleScroll}
    >
      {children}
    </div>
  )
}
