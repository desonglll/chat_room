/**
 * The browser side of the autoplay policy: `prefers-reduced-motion` and Save-Data, read
 * live (both can change while the page is open) and shared by every GIF on the page.
 */
import { useSyncExternalStore } from 'react'
import { autoplayAllowed, type AutoplayEnvironment } from './playback'

interface NetworkInformationLike extends EventTarget {
  saveData?: boolean
}

function connection(): NetworkInformationLike | undefined {
  if (typeof navigator === 'undefined') return undefined
  return (navigator as Navigator & { connection?: NetworkInformationLike }).connection
}

function reducedMotionQuery(): MediaQueryList | undefined {
  if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') return undefined
  return window.matchMedia('(prefers-reduced-motion: reduce)')
}

export function readAutoplayEnvironment(): AutoplayEnvironment {
  return {
    reducedMotion: reducedMotionQuery()?.matches ?? false,
    saveData: connection()?.saveData === true,
  }
}

function subscribe(onChange: () => void): () => void {
  const query = reducedMotionQuery()
  const network = connection()
  query?.addEventListener('change', onChange)
  network?.addEventListener('change', onChange)
  return () => {
    query?.removeEventListener('change', onChange)
    network?.removeEventListener('change', onChange)
  }
}

const snapshot = () => autoplayAllowed(readAutoplayEnvironment())
/** Server rendering and tests without a window: never autoplay. */
const serverSnapshot = () => false

export function useAutoplayAllowed(): boolean {
  return useSyncExternalStore(subscribe, snapshot, serverSnapshot)
}
