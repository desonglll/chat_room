/**
 * Page-wide inputs to the playback policy: tab visibility, `prefers-reduced-motion`, and the
 * device pixel ratio. An interface so the manager is testable without a DOM.
 */

export interface PageSignals {
  hidden(): boolean
  reducedMotion(): boolean
  pixelRatio(): number
  /** Called whenever any signal changes. Returns an unsubscribe. */
  subscribe(listener: () => void): () => void
}

/** Stickers never render above 2× — beyond that the cost quadruples for no visible gain. */
export const MAX_PIXEL_RATIO = 2

export function browserPageSignals(): PageSignals {
  const motion = window.matchMedia('(prefers-reduced-motion: reduce)')
  return {
    hidden: () => document.visibilityState === 'hidden',
    reducedMotion: () => motion.matches,
    pixelRatio: () => Math.min(MAX_PIXEL_RATIO, Math.max(1, window.devicePixelRatio || 1)),
    subscribe(listener) {
      document.addEventListener('visibilitychange', listener)
      motion.addEventListener('change', listener)
      return () => {
        document.removeEventListener('visibilitychange', listener)
        motion.removeEventListener('change', listener)
      }
    },
  }
}
