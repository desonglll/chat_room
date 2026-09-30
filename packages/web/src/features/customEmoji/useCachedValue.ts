import { useEffect, useSyncExternalStore } from 'react'
import type { BatchCache } from './batchCache'

/**
 * Subscribe to one key of a batch cache. The request is issued from an effect, so server
 * rendering and tests never start a fetch; `undefined` means "not known yet".
 */
export function useCachedValue<V>(cache: BatchCache<V>, key: string): V | null | undefined {
  const value = useSyncExternalStore(
    cache.subscribe,
    () => cache.get(key),
    () => cache.get(key),
  )
  useEffect(() => {
    cache.request(key)
  }, [cache, key])
  return value
}
