import { useEffect, useState } from 'react'

/**
 * Bridge from the token layer to a JS animation library.
 *
 * `@tg/ui` itself animates entirely in CSS (see docs/devlog/TG-010.md § Decisions), so nothing in
 * this package calls these. They exist because TG-009 exported the springs as bare
 * mass/stiffness/damping numbers precisely so that `motion`/framer-motion consumers in M1 read
 * the tokens instead of retyping the numbers, and that bridge has to live somewhere that is not
 * a feature module.
 *
 * `null` means the token layer is not loaded. No fallback numbers are returned on purpose:
 * duplicating TG-009's values here would create a second source of truth that silently drifts.
 */
export interface SpringTokens {
  mass: number
  stiffness: number
  damping: number
}

export type SpringName = 'standard' | 'smooth' | 'bounce'

const PREFIX: Record<SpringName, string> = {
  standard: '--tg-spring',
  smooth: '--tg-spring-smooth',
  bounce: '--tg-spring-bounce',
}

function readNumber(styles: CSSStyleDeclaration, name: string): number | null {
  const raw = styles.getPropertyValue(name).trim()
  if (raw === '') return null
  const value = Number.parseFloat(raw)
  return Number.isFinite(value) ? value : null
}

/**
 * Reads one of TG-009's three springs. Pass the element the animation runs on, so that a
 * `prefers-reduced-motion` override or a scoped theme is picked up.
 */
export function readSpring(name: SpringName, from?: Element | null): SpringTokens | null {
  if (typeof window === 'undefined') return null
  const element = from ?? document.documentElement
  const styles = window.getComputedStyle(element)
  const prefix = PREFIX[name]
  const mass = readNumber(styles, `${prefix}-mass`)
  const stiffness = readNumber(styles, `${prefix}-stiffness`)
  const damping = readNumber(styles, `${prefix}-damping`)
  if (mass === null || stiffness === null || damping === null) return null
  return { mass, stiffness, damping }
}

/**
 * For the rare case that has to be expressed in JS. CSS-driven motion in this package needs no
 * such check: TG-009 put the degradation in `semantic.css`, so the durations and easings are
 * already reduced by the time a component reads them.
 */
export function usePrefersReducedMotion(): boolean {
  const [reduced, setReduced] = useState(false)

  useEffect(() => {
    const query = window.matchMedia('(prefers-reduced-motion: reduce)')
    setReduced(query.matches)
    const onChange = (event: MediaQueryListEvent) => setReduced(event.matches)
    query.addEventListener('change', onChange)
    return () => query.removeEventListener('change', onChange)
  }, [])

  return reduced
}
