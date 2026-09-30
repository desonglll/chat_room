/** Test-only fake `CoreClock` with ordered timer firing (shared by this feature's tests). */
import type { CoreClock } from '@tg/core'

export class FakeClock implements CoreClock {
  nowMs: number
  private timers: Array<{ id: number; at: number; callback: () => void }> = []
  private nextId = 1

  constructor(start = 0) {
    this.nowMs = start
  }

  now = () => this.nowMs

  setTimeout = (callback: () => void, delayMs: number) => {
    const id = this.nextId++
    this.timers.push({ id, at: this.nowMs + delayMs, callback })
    return id
  }

  clearTimeout = (handle: unknown) => {
    this.timers = this.timers.filter((timer) => timer.id !== handle)
  }

  pending = () => this.timers.length

  advance(ms: number) {
    const target = this.nowMs + ms
    for (;;) {
      const due = this.timers.filter((timer) => timer.at <= target).sort((a, b) => a.at - b.at)[0]
      if (!due) break
      this.timers = this.timers.filter((timer) => timer !== due)
      this.nowMs = due.at
      due.callback()
    }
    this.nowMs = target
  }
}
