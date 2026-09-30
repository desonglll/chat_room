/**
 * The viewer's single animation primitive: drive a 0→1 progress with TG-009's `smooth`
 * spring (critically damped, ~390 ms, no overshoot — the iOS 26 curve), or — under
 * `prefers-reduced-motion` — with a short linear fade. Every moving part of the viewer
 * (shared-element flight, slide, zoom settle) is expressed as an interpolation over it, so
 * reduced motion is decided in exactly one place.
 */
import { animate } from 'motion'
import { readSpring } from '@tg/ui'

export interface Progress {
  /** Resolves true when the animation ran to its end, false when `stop()` interrupted it. */
  finished: Promise<boolean>
  stop(): void
}

/** `--tg-duration-fade` in ms (it survives reduced motion by design), 150 if unreadable. */
export function fadeDurationMs(from?: Element | null): number {
  if (typeof window === 'undefined') return 150
  const raw = window
    .getComputedStyle(from ?? document.documentElement)
    .getPropertyValue('--tg-duration-fade')
    .trim()
  const value = Number.parseFloat(raw)
  if (!Number.isFinite(value)) return 150
  return raw.endsWith('ms') ? value : value * 1000
}

export function runProgress(
  onUpdate: (t: number) => void,
  options: { reduced: boolean; from?: Element | null },
): Progress {
  const spring = options.reduced ? null : readSpring('smooth', options.from)
  const controls = animate(
    0,
    1,
    spring
      ? { type: 'spring', ...spring, restDelta: 0.001, onUpdate }
      : { type: 'tween', ease: 'linear', duration: fadeDurationMs(options.from) / 1000, onUpdate },
  )
  let stopped = false
  return {
    // Land exactly on 1 (a spring rests within `restDelta`), unless interrupted.
    finished: Promise.resolve(controls.finished).then(
      () => {
        if (stopped) return false
        onUpdate(1)
        return true
      },
      () => false,
    ),
    stop: () => {
      stopped = true
      controls.stop()
    },
  }
}
