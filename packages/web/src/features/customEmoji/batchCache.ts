/**
 * A keyed lookup cache that batches misses into one request, for data many small
 * components ask for at once (every inline custom emoji of a screen, every name's emoji
 * status). Components subscribe with `useSyncExternalStore`; a finished batch notifies
 * once. `undefined` = not asked yet or in flight, `null` = the server does not know it.
 */
export interface BatchCacheOptions<V> {
  /** Load a batch; keys missing from the answer are cached as `null`. */
  load(keys: string[]): Promise<Map<string, V>>
  maxBatch: number
  /** How long an answer (including `null`) is trusted. `Infinity` = for the page's life. */
  ttlMs: number
  now(): number
  /** Defer the flush so one render's requests share a batch. */
  schedule(flush: () => void): void
}

export interface BatchCache<V> {
  get(key: string): V | null | undefined
  /** Ask for `key` if it is not cached (or has gone stale); cheap to call every render. */
  request(key: string): void
  /** Put known answers in (the picker's own list, a status just set) without a request. */
  seed(entries: Iterable<[string, V | null]>): void
  invalidate(key: string): void
  subscribe(listener: () => void): () => void
}

interface Entry<V> {
  value: V | null
  at: number
}

export function createBatchCache<V>(options: BatchCacheOptions<V>): BatchCache<V> {
  const entries = new Map<string, Entry<V>>()
  const inFlight = new Set<string>()
  const queued = new Set<string>()
  const listeners = new Set<() => void>()
  let scheduled = false

  const fresh = (entry: Entry<V> | undefined) => entry !== undefined && options.now() - entry.at < options.ttlMs
  const notify = () => listeners.forEach((listener) => listener())

  const flush = () => {
    scheduled = false
    const keys = [...queued]
    queued.clear()
    for (let start = 0; start < keys.length; start += options.maxBatch) {
      const batch = keys.slice(start, start + options.maxBatch)
      batch.forEach((key) => inFlight.add(key))
      options
        .load(batch)
        .then(
          (found) => {
            const at = options.now()
            for (const key of batch) entries.set(key, { value: found.get(key) ?? null, at })
          },
          () => {
            // A failed batch is not an answer: leave the keys unknown so a later render retries.
          },
        )
        .finally(() => {
          batch.forEach((key) => inFlight.delete(key))
          notify()
        })
    }
  }

  return {
    get: (key) => entries.get(key)?.value,
    request(key) {
      if (key === '' || fresh(entries.get(key)) || inFlight.has(key) || queued.has(key)) return
      queued.add(key)
      if (!scheduled) {
        scheduled = true
        options.schedule(flush)
      }
    },
    seed(values) {
      const at = options.now()
      for (const [key, value] of values) entries.set(key, { value, at })
      notify()
    },
    invalidate(key) {
      entries.delete(key)
      notify()
    },
    subscribe(listener) {
      listeners.add(listener)
      return () => listeners.delete(listener)
    },
  }
}
