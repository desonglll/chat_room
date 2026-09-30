/**
 * One IntersectionObserver for every GIF on the page. A GIF counts as visible only while
 * part of it is actually inside the viewport (no margin): off-screen GIFs must stop
 * decoding, which is the card's acceptance criterion.
 */

type Listener = (visible: boolean) => void

let observer: IntersectionObserver | null = null
const listeners = new Map<Element, Listener>()

export function observeGifVisibility(element: Element, onChange: Listener): () => void {
  if (typeof IntersectionObserver === 'undefined') {
    // No observer (old engines, tests): treat as visible so tap-to-play still works.
    onChange(true)
    return () => undefined
  }
  observer ??= new IntersectionObserver((entries) => {
    for (const entry of entries) listeners.get(entry.target)?.(entry.isIntersecting)
  })
  listeners.set(element, onChange)
  observer.observe(element)
  return () => {
    listeners.delete(element)
    observer?.unobserve(element)
  }
}
