import { useEffect, type RefObject } from 'react'
import { collectFocusable, describe } from './domFocus'
import { nextTabStop } from './focusOrder'

/**
 * Moves focus into a subtree on open and, when `trap` is on, confines Tab to it.
 *
 * Callers that must not touch focus at all (`Tooltip`) pass `active: false` rather than setting
 * `trap: false` - moving focus into a tooltip would blur its own trigger and close it.
 *
 * Three behaviours, all required by the TG-010 acceptance criteria:
 *  - entry: focus moves into the container on activation (`initialFocusRef`, else the first tab
 *    stop, else the container itself, which is why callers give it `tabIndex={-1}`);
 *  - cycling: Tab from the last stop wraps to the first, Shift+Tab from the first wraps to the
 *    last, using the pure `nextTabStop`;
 *  - recovery: a `focusin` anywhere outside the container pulls focus back. This is the net for
 *    focus that arrives by means other than Tab — a click on the backdrop, `element.focus()`
 *    from application code, or a browser control returning focus to the document.
 *
 * Restoration is NOT here. `useRestoreFocus` owns it, because an overlay that manages focus
 * without trapping it (a `Menu`, which must let Tab dismiss it) still has to give focus back.
 */
export interface FocusTrapOptions {
  active: boolean
  containerRef: RefObject<HTMLElement | null>
  initialFocusRef?: RefObject<HTMLElement | null> | undefined
  /** Set false to keep Tab free while still moving focus in on open. */
  trap?: boolean | undefined
}

export function useFocusTrap({ active, containerRef, initialFocusRef, trap = true }: FocusTrapOptions): void {
  useEffect(() => {
    const container = containerRef.current
    if (!active || container === null) return

    // Entry uses the same rule as cycling, so the container itself (tabIndex -1) is never the
    // landing spot while a real control exists inside it. Getting this wrong is subtle and
    // expensive: focus lands on the panel, the first Arrow key goes nowhere, and the component
    // looks keyboard-inert even though every handler is wired.
    const entryCandidates = collectFocusable(container)
    const entryIndex = nextTabStop(entryCandidates.map(describe), -1, false)
    const enter = initialFocusRef?.current ?? (entryIndex === -1 ? null : entryCandidates[entryIndex])
    if (enter === null || enter === undefined) container.focus()
    else enter.focus()

    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== 'Tab' || event.defaultPrevented) return
      const elements = collectFocusable(container)
      const candidates = elements.map(describe)
      const current = elements.findIndex((element) => element === document.activeElement)
      const target = nextTabStop(candidates, current, event.shiftKey)
      event.preventDefault()
      if (target === -1) {
        container.focus()
        return
      }
      elements[target]?.focus()
    }

    const onFocusIn = (event: FocusEvent) => {
      const target = event.target
      if (!(target instanceof Node) || container.contains(target)) return
      const elements = collectFocusable(container)
      const first = nextTabStop(elements.map(describe), -1, false)
      if (first === -1) container.focus()
      else elements[first]?.focus()
    }

    if (!trap) return

    container.addEventListener('keydown', onKeyDown)
    document.addEventListener('focusin', onFocusIn)
    return () => {
      container.removeEventListener('keydown', onKeyDown)
      document.removeEventListener('focusin', onFocusIn)
    }
  }, [active, containerRef, initialFocusRef, trap])
}
