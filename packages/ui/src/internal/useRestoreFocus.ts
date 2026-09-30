import { useEffect, useRef } from 'react'

/**
 * Gives focus back to whatever had it before an overlay opened.
 *
 * Separate from `useFocusTrap` because the two are not the same concern: a `Menu` must NOT trap
 * Tab (the ARIA menu pattern makes Tab dismiss it) but must still return focus to its trigger.
 * Bundling restoration into the trap would silently drop it for every untrapped overlay.
 *
 * Two ordering rules, both learned the hard way against a real browser:
 *
 *  - the element is captured DURING RENDER, not in an effect. Effects run after the focus trap has
 *    already moved focus into the overlay, so an effect would capture a control inside the
 *    overlay, which is then unmounted, and nothing would be restored.
 *  - restoration happens in an effect cleanup, and this hook must be declared AFTER `useFocusTrap`.
 *    Cleanups run in declaration order, so the trap removes its `focusin` recovery listener first;
 *    otherwise that listener would see focus leaving and immediately drag it back.
 */
export function useRestoreFocus(active: boolean, enabled = true): void {
  const previous = useRef<HTMLElement | null>(null)
  const wasActive = useRef(false)

  if (wasActive.current !== active) {
    wasActive.current = active
    if (active && typeof document !== 'undefined') {
      previous.current = document.activeElement instanceof HTMLElement ? document.activeElement : null
    }
  }

  useEffect(() => {
    if (!active || !enabled) return
    return () => {
      const target = previous.current
      previous.current = null
      // isConnected: the trigger may itself have been unmounted by whatever the overlay did.
      if (target !== null && target.isConnected) target.focus()
    }
  }, [active, enabled])
}
