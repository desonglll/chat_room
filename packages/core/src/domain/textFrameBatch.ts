/**
 * Coalesce streamed text into one flush per animation frame. Migrated from
 * `web/src/textFrameBatch.ts` (TG-011) with one change: the scheduler is REQUIRED. The
 * old defaults (`requestAnimationFrame`/`cancelAnimationFrame`) do not exist off-DOM, so
 * a default that only works in a browser would be a trap; the web host passes rAF, other
 * hosts pass their clock.
 */
export interface TextFrameBatch {
  push(text: string): void
  flush(): void
}

export interface FrameScheduler {
  schedule(callback: () => void): unknown
  cancel(handle: unknown): void
}

export function createTextFrameBatch(onFlush: (text: string) => void, scheduler: FrameScheduler): TextFrameBatch {
  let buffer = ''
  let frame: unknown = null

  function flush(): void {
    if (frame !== null) scheduler.cancel(frame)
    frame = null
    if (!buffer) return
    const text = buffer
    buffer = ''
    onFlush(text)
  }

  return {
    push(text) {
      buffer += text
      if (frame !== null) return
      frame = scheduler.schedule(() => flush())
    },
    flush,
  }
}
