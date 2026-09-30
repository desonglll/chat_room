/**
 * In-page frame and long-task sampling for the benchmark. `measure.mjs` calls
 * `window.__stickerBench.sample(ms)` and combines the result with CDP CPU metrics.
 */

export interface FrameSample {
  durationMs: number
  frames: number
  fps: number
  /** 95th / 99th percentile rAF interval, ms. */
  p95FrameMs: number
  p99FrameMs: number
  /** rAF intervals longer than 1.5 × 16.7 ms, i.e. at least one dropped frame. */
  droppedIntervals: number
  longTasks: number
  longTaskMs: number
}

const percentile = (sorted: number[], p: number) =>
  sorted.length === 0 ? 0 : (sorted[Math.min(sorted.length - 1, Math.floor(sorted.length * p))] ?? 0)

export function sampleFrames(durationMs: number): Promise<FrameSample> {
  return new Promise((resolve) => {
    const intervals: number[] = []
    let longTasks = 0
    let longTaskMs = 0
    const observer = new PerformanceObserver((list) => {
      for (const entry of list.getEntries()) {
        longTasks += 1
        longTaskMs += entry.duration
      }
    })
    try {
      observer.observe({ type: 'longtask', buffered: false })
    } catch {
      // longtask is Chromium-only; Firefox/Safari report 0.
    }
    const start = performance.now()
    let last = start
    const tick = (now: number) => {
      intervals.push(now - last)
      last = now
      if (now - start < durationMs) {
        requestAnimationFrame(tick)
        return
      }
      observer.disconnect()
      const elapsed = now - start
      const sorted = intervals.slice(1).sort((a, b) => a - b)
      resolve({
        durationMs: Math.round(elapsed),
        frames: intervals.length - 1,
        fps: Math.round(((intervals.length - 1) / elapsed) * 1000 * 10) / 10,
        p95FrameMs: Math.round(percentile(sorted, 0.95) * 10) / 10,
        p99FrameMs: Math.round(percentile(sorted, 0.99) * 10) / 10,
        droppedIntervals: sorted.filter((ms) => ms > 25).length,
        longTasks,
        longTaskMs: Math.round(longTaskMs),
      })
    }
    requestAnimationFrame((now) => {
      last = now
      requestAnimationFrame(tick)
    })
  })
}
