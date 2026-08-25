export interface TextFrameBatch {
  push(text: string): void
  flush(): void
}

export function createTextFrameBatch(
  onFlush: (text: string) => void,
  schedule: (callback: FrameRequestCallback) => number = requestAnimationFrame,
  cancel: (handle: number) => void = cancelAnimationFrame,
): TextFrameBatch {
  let buffer = ''
  let frame: number | null = null

  function flush(): void {
    if (frame !== null) cancel(frame)
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
      frame = schedule(() => flush())
    },
    flush,
  }
}
