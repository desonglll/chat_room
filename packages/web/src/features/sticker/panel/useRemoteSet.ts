/**
 * Resolves a typed short name to a not-yet-installed set (`GET /api/sticker-sets/:name`),
 * debounced so typing "cats" does not fetch "c", "ca" and "cat".
 */
import { useEffect, useState } from 'react'
import type { StickerSet } from '@tg/core'
import { stickerLibrary } from '../stickerLibrary'

export const REMOTE_SET_DEBOUNCE_MS = 350

export function useRemoteSet(shortName: string | null): StickerSet | null {
  const [found, setFound] = useState<StickerSet | null>(null)
  useEffect(() => {
    setFound(null)
    if (!shortName) return
    let cancelled = false
    const timer = window.setTimeout(() => {
      stickerLibrary()
        .lookupSet(shortName)
        .then((set) => {
          if (!cancelled && set && set.set_type === 'regular') setFound(set)
        })
        .catch(() => undefined)
    }, REMOTE_SET_DEBOUNCE_MS)
    return () => {
      cancelled = true
      window.clearTimeout(timer)
    }
  }, [shortName])
  return found
}
