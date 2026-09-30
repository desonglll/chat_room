/**
 * TG-101 performance budget, executable. Run:
 *
 *   bun packages/core/src/domain/messageViewportBenchmark.ts
 *
 * It prints one line per measurement and exits 1 when any exceeds its budget. The budgets
 * and their reasoning live in docs/devlog/TG-101.md ("Performance budget"); the numbers
 * here must match that table. Dev-only: uses `bun:jsc` and is NOT exported from the domain
 * index, so it never reaches a bundle or a non-Bun runtime. `messageViewportBenchmark.test.ts` runs the same code with a
 * 5x slack so a CI box that is slower than the dev machine only fails on an algorithmic
 * regression (say O(n) → O(n²)), not on noise.
 */
import { heapStats } from 'bun:jsc'
import type { BroadcastMessage, DisplayMessage } from './messageView'
import { computeMessageLayout, createMessageLayoutCache, messageKey } from './messageViewportLayout'
import { countUnreadBelow, mergeMessagePages } from './messageViewportWindow'

export const MESSAGE_COUNT = 100_000
const PAGE = 50

/** Budgets in milliseconds (median of the runs), and bytes per message for memory. */
export const BUDGET = {
  layoutCold100k: 120,
  layoutWarm100k: 12,
  prependPageInto100k: 8,
  unreadScan: 1,
  layoutHeapBytesPerMessage: 256,
} as const

export interface BenchResult {
  name: keyof typeof BUDGET
  value: number
  budget: number
  unit: 'ms' | 'B/msg'
}

const BASE = Date.parse('2026-01-01T00:00:00Z')

export function syntheticMessages(count: number, start = 0): BroadcastMessage[] {
  const out: BroadcastMessage[] = new Array(count)
  for (let i = 0; i < count; i += 1) {
    const n = start + i
    // Runs of 1-4 messages from 7 senders, ~40 s apart: ~100 days for 100k messages.
    const sender = `u${Math.floor(n / (1 + (n % 4))) % 7}`
    out[i] = {
      type: 'broadcast',
      message_id: `m${String(n).padStart(8, '0')}`,
      sender_id: sender,
      sender,
      sender_avatar: '',
      content: `message ${n}`,
      attachment: null,
      reply_to: null,
      recalled_at: null,
      edited_at: null,
      timestamp: new Date(BASE + n * 40_000).toISOString(),
      favorite_id: null,
      forwarded_from: null,
      reactions: [],
    }
  }
  return out
}

function median(samples: number[]): number {
  const sorted = samples.slice().sort((a, b) => a - b)
  return sorted[Math.floor(sorted.length / 2)] as number
}

function time(runs: number, body: () => void): number {
  const samples: number[] = []
  for (let run = 0; run < runs; run += 1) {
    const start = performance.now()
    body()
    samples.push(performance.now() - start)
  }
  return median(samples)
}

const layoutOptions = { currentUserId: 'u0', showGroupIdentity: true }

export function runMessageViewportBenchmark(count = MESSAGE_COUNT): BenchResult[] {
  const results: BenchResult[] = []
  const push = (name: keyof typeof BUDGET, value: number, unit: BenchResult['unit'] = 'ms') =>
    results.push({ name, value, budget: BUDGET[name], unit })

  // Cold: fresh objects every run, so every timestamp is parsed and every day computed.
  const coldSets = Array.from({ length: 5 }, () => syntheticMessages(count))
  let coldIndex = 0
  push(
    'layoutCold100k',
    time(5, () => {
      computeMessageLayout(coldSets[coldIndex++] as BroadcastMessage[], layoutOptions)
    }),
  )

  // Warm: the append path — same objects plus one new message, caches populated.
  // Warm: the append path through the incremental cache — a new array holding the same
  // objects plus one new message, exactly what a store update produces.
  const cache = createMessageLayoutCache()
  let warm: DisplayMessage[] = syntheticMessages(count)
  cache(warm, layoutOptions)
  let next = count
  push(
    'layoutWarm100k',
    time(15, () => {
      warm = warm.concat(syntheticMessages(1, next++))
      cache(warm, layoutOptions)
    }),
  )

  // Prepend: build the 100k loaded page by page, oldest last, and time each page merge.
  let loaded: BroadcastMessage[] = []
  const pageSamples: number[] = []
  for (let first = count - PAGE; first >= 0; first -= PAGE) {
    const page = syntheticMessages(PAGE + 1, first) // +1: the inclusive-cursor duplicate
    const start = performance.now()
    loaded = mergeMessagePages(loaded, page).messages
    pageSamples.push(performance.now() - start)
  }
  push('prependPageInto100k', median(pageSamples.slice(-40)))

  const seen = messageKey(loaded[loaded.length - 30] as BroadcastMessage)
  push(
    'unreadScan',
    time(21, () => {
      countUnreadBelow(loaded, seen, 'u0', messageKey)
    }),
  )

  // Layout overhead per message (the messages themselves are the store's, not ours).
  const held = syntheticMessages(count)
  Bun.gc(true)
  const heapBefore = heapStats().heapSize
  const heldLayout = computeMessageLayout(held, layoutOptions)
  Bun.gc(true)
  push('layoutHeapBytesPerMessage', Math.max(0, heapStats().heapSize - heapBefore) / count, 'B/msg')
  if (heldLayout.length !== held.length) throw new Error('layout length mismatch')
  return results
}

export function formatBenchResult(result: BenchResult, slack = 1): string {
  const ok = result.value <= result.budget * slack
  const value = result.unit === 'ms' ? result.value.toFixed(2) : Math.round(result.value).toString()
  return `${ok ? 'PASS' : 'FAIL'} ${result.name.padEnd(22)} ${value.padStart(9)} ${result.unit}  (budget ${result.budget * slack} ${result.unit})`
}

if (import.meta.main) {
  const results = runMessageViewportBenchmark()
  for (const result of results) console.log(formatBenchResult(result))
  if (results.some((result) => result.value > result.budget)) process.exit(1)
}
