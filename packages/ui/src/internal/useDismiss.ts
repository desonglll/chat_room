import { useEffect, useRef, type RefObject } from 'react'
import { isTopLayer, pushLayer, removeLayer, type LayerId } from './layerStack'

/**
 * Escape-to-close and pointer-outside-to-close, layer aware.
 *
 * Escape only fires for the topmost layer (see `layerStack.ts`), so a `Menu` inside a `Modal`
 * takes the first Escape and the `Modal` takes the second, which is what every desktop client
 * including Telegram does. Outside-pointer uses `pointerdown` rather than `click`: by the time a
 * `click` completes the user may already have pressed a button in the layer underneath.
 */
export interface DismissOptions {
  active: boolean
  onDismiss: (reason: 'escape' | 'outside-pointer') => void
  /** Elements that count as "inside": the overlay surface, plus its anchor or trigger. */
  refs: readonly (RefObject<HTMLElement | null> | undefined)[]
  closeOnEscape?: boolean | undefined
  closeOnOutsidePointer?: boolean | undefined
}

export function useDismiss({
  active,
  onDismiss,
  refs,
  closeOnEscape = true,
  closeOnOutsidePointer = true,
}: DismissOptions): void {
  // The callback is read from a ref so that an inline arrow function in the consumer does not
  // tear down and rebuild the listeners (and with them the layer registration) on every render.
  const handler = useRef(onDismiss)
  handler.current = onDismiss

  const insideRefs = useRef(refs)
  insideRefs.current = refs

  useEffect(() => {
    if (!active) return
    const layer: LayerId = pushLayer()

    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== 'Escape' || !closeOnEscape) return
      if (!isTopLayer(layer) || event.defaultPrevented) return
      event.preventDefault()
      event.stopPropagation()
      handler.current('escape')
    }

    const onPointerDown = (event: PointerEvent) => {
      if (!closeOnOutsidePointer || !isTopLayer(layer)) return
      const target = event.target
      if (!(target instanceof Node)) return
      for (const ref of insideRefs.current) {
        const element = ref?.current
        if (element !== null && element !== undefined && element.contains(target)) return
      }
      handler.current('outside-pointer')
    }

    document.addEventListener('keydown', onKeyDown, true)
    document.addEventListener('pointerdown', onPointerDown, true)
    return () => {
      document.removeEventListener('keydown', onKeyDown, true)
      document.removeEventListener('pointerdown', onPointerDown, true)
      removeLayer(layer)
    }
  }, [active, closeOnEscape, closeOnOutsidePointer])
}
