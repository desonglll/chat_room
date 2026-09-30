/**
 * Page signals for the round video bubble (TG-402): whether an element is at least half in
 * the viewport, and whether the user asked for reduced motion (then nothing autoplays).
 */
import { useEffect, useState, type RefObject } from 'react'

export function useInViewport(ref: RefObject<Element | null>, threshold = 0.5): boolean {
  const [inView, setInView] = useState(false)
  useEffect(() => {
    const node = ref.current
    if (!node || typeof IntersectionObserver === 'undefined') return
    const observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) setInView(entry.isIntersecting && entry.intersectionRatio >= threshold)
      },
      { threshold: [0, threshold, 1] },
    )
    observer.observe(node)
    return () => observer.disconnect()
  }, [ref, threshold])
  return inView
}

const REDUCED_MOTION = '(prefers-reduced-motion: reduce)'

export function usePrefersReducedMotion(): boolean {
  const [reduced, setReduced] = useState(
    () =>
      typeof window !== 'undefined' &&
      typeof window.matchMedia === 'function' &&
      window.matchMedia(REDUCED_MOTION).matches,
  )
  useEffect(() => {
    if (typeof window.matchMedia !== 'function') return
    const query = window.matchMedia(REDUCED_MOTION)
    const onChange = () => setReduced(query.matches)
    query.addEventListener('change', onChange)
    return () => query.removeEventListener('change', onChange)
  }, [])
  return reduced
}
