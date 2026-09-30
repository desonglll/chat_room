/**
 * Keyed async values shared by reference count, with LRU eviction of unreferenced entries
 * beyond a cost ceiling. Backs both the parse cache (one parse per sticker, however many
 * instances) and the first-frame snapshot cache (object URLs revoked on eviction).
 *
 * A failed value is dropped immediately so the next acquire retries.
 */

export interface RefCountedCacheOptions<V> {
  /** Eviction starts above this total cost of *unreferenced* entries. */
  maxIdleCost: number
  cost(value: V): number
  onEvict?(value: V): void
}

interface Entry<V> {
  promise: Promise<V>
  value: V | undefined
  refs: number
  cost: number
}

export interface RefCountedCache<V> {
  acquire(key: string, create: () => Promise<V>): Promise<V>
  release(key: string): void
  /** Settled value without taking a reference, if present. */
  peek(key: string): V | undefined
  readonly size: number
}

export function createRefCountedCache<V>(options: RefCountedCacheOptions<V>): RefCountedCache<V> {
  // Map iteration order doubles as the LRU order of idle entries (re-inserted on release).
  const entries = new Map<string, Entry<V>>()

  const trim = () => {
    let idle = 0
    for (const entry of entries.values()) if (entry.refs === 0) idle += entry.cost
    for (const [key, entry] of entries) {
      if (idle <= options.maxIdleCost) break
      if (entry.refs > 0 || entry.value === undefined) continue
      entries.delete(key)
      idle -= entry.cost
      options.onEvict?.(entry.value)
    }
  }

  return {
    acquire(key, create) {
      const existing = entries.get(key)
      if (existing) {
        existing.refs += 1
        return existing.promise
      }
      const entry: Entry<V> = { promise: Promise.resolve() as Promise<V>, value: undefined, refs: 1, cost: 0 }
      entry.promise = create().then(
        (value) => {
          entry.value = value
          entry.cost = options.cost(value)
          trim()
          return value
        },
        (error: unknown) => {
          if (entries.get(key) === entry) entries.delete(key)
          throw error
        },
      )
      entries.set(key, entry)
      return entry.promise
    },
    release(key) {
      const entry = entries.get(key)
      if (!entry || entry.refs === 0) return
      entry.refs -= 1
      if (entry.refs > 0) return
      entries.delete(key)
      entries.set(key, entry)
      trim()
    },
    peek: (key) => entries.get(key)?.value,
    get size() {
      return entries.size
    },
  }
}
