/**
 * Reconnect backoff schedule. This is the frozen Vue client's exact curve
 * (`min(500 * 2^attempt, 5000)` ms — `web/src/composables/useChatSocket.ts`), kept so the
 * rewrite does not change reconnect pressure on the server; deterministic on purpose,
 * with optional jitter for hosts that want to spread thundering herds.
 */
export interface BackoffOptions {
  baseMs?: number
  capMs?: number
  /** 0..1 random source; when provided, up to ±25% jitter is applied. */
  random?: () => number
}

export function reconnectDelayMs(attempt: number, options: BackoffOptions = {}): number {
  const base = options.baseMs ?? 500
  const cap = options.capMs ?? 5_000
  const raw = Math.min(base * 2 ** Math.max(0, attempt), cap)
  if (!options.random) return raw
  const jitter = (options.random() - 0.5) * 0.5 // ±25%
  return Math.round(raw * (1 + jitter))
}
