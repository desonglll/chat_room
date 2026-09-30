/**
 * One IntersectionObserver for every sticker on the page (one observer per element costs
 * more and delivers the same entries). Injectable so the manager is testable without a DOM.
 */

export interface ViewportWatcher {
  observe(element: Element, onChange: (visible: boolean) => void): () => void
}

/** Start decoding and rendering slightly before a sticker scrolls in. */
export const VIEWPORT_MARGIN = '100px 0px'

export function browserViewportWatcher(): ViewportWatcher {
  const listeners = new Map<Element, (visible: boolean) => void>()
  const observer = new IntersectionObserver(
    (entries) => {
      for (const entry of entries) listeners.get(entry.target)?.(entry.isIntersecting)
    },
    { rootMargin: VIEWPORT_MARGIN },
  )
  return {
    observe(element, onChange) {
      listeners.set(element, onChange)
      observer.observe(element)
      return () => {
        listeners.delete(element)
        observer.unobserve(element)
      }
    },
  }
}
