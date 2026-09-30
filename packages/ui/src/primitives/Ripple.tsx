import { useCallback, useEffect, useRef, useState } from 'react'
import { cx } from '../internal/cx'
import { hostCentre, rippleGeometry } from '../internal/rippleGeometry'
import type { Tint } from './types'

/**
 * Telegram's touch feedback. Drop it as a child of any `position: relative` element:
 *
 *     <button className="tg-button"> <Ripple /> Send </button>
 *
 * It subscribes to its own `parentElement` rather than taking a ref, so a control does not have
 * to thread an extra ref through its props, and it never intercepts the pointer itself
 * (`pointer-events: none`) so the host's own handlers are unaffected.
 *
 * It is NOT a Material ripple. See `internal/rippleGeometry.ts` and the devlog: the wave is
 * centred on the pointer and stays there instead of migrating to the element centre, it is
 * sized from the host's larger side instead of from the diagonal, and it runs for
 * `--tg-ripple-duration` (700ms) to completion rather than being cut short on release.
 */
export interface RippleProps {
  /** Which token colours the wave. `inverse` is for a wave on top of an accent fill. */
  tint?: Tint | undefined
  disabled?: boolean | undefined
  /** Telegram's handheld variant: starts at 27% instead of 0, so it reads on a small target. */
  compact?: boolean | undefined
  className?: string | undefined
}

interface Wave {
  id: number
  left: number
  top: number
  diameter: number
}

export function Ripple({ tint = 'default', disabled = false, compact = false, className }: RippleProps) {
  const surface = useRef<HTMLSpanElement>(null)
  const nextId = useRef(0)
  const [waves, setWaves] = useState<Wave[]>([])

  const retire = useCallback((id: number) => {
    setWaves((current) => current.filter((wave) => wave.id !== id))
  }, [])

  useEffect(() => {
    const host = surface.current?.parentElement ?? null
    if (host === null || disabled) return

    const spawn = (clientX: number | null, clientY: number | null) => {
      const rect = host.getBoundingClientRect()
      const size = { width: rect.width, height: rect.height }
      const pointer =
        clientX === null || clientY === null ? hostCentre(size) : { x: clientX - rect.left, y: clientY - rect.top }
      const geometry = rippleGeometry({ host: size, pointer })
      nextId.current += 1
      setWaves((current) => [...current, { id: nextId.current, ...geometry }])
    }

    const onPointerDown = (event: PointerEvent) => {
      // Primary button / primary touch contact only: a right-click opens a context menu and a
      // ripple there would read as an activation that did not happen.
      if (event.button !== 0) return
      spawn(event.clientX, event.clientY)
    }

    // Keyboard activation has no pointer, so the wave starts at the centre. Without this, a
    // keyboard user gets no press feedback at all.
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.repeat) return
      if (event.key !== 'Enter' && event.key !== ' ') return
      spawn(null, null)
    }

    host.addEventListener('pointerdown', onPointerDown)
    host.addEventListener('keydown', onKeyDown)
    return () => {
      host.removeEventListener('pointerdown', onPointerDown)
      host.removeEventListener('keydown', onKeyDown)
    }
  }, [disabled])

  return (
    <span
      ref={surface}
      aria-hidden="true"
      data-tg-ripple=""
      className={cx(
        'tg-ripple',
        compact && 'tg-ripple--compact',
        tint !== 'default' && `tg-ripple--${tint}`,
        className,
      )}
    >
      {waves.map((wave) => (
        <span
          key={wave.id}
          className="tg-ripple__wave"
          style={{ left: wave.left, top: wave.top, width: wave.diameter, height: wave.diameter }}
          onAnimationEnd={() => retire(wave.id)}
        />
      ))}
    </span>
  )
}
