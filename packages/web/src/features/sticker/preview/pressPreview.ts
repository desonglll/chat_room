/**
 * Telegram's press-and-hold sticker preview, as a pure state machine over a clock.
 *
 *   press on a sticker ──(hold ≥ holdMs, moved < slop)──▶ preview open on that sticker
 *   while open, the pointer moving over another sticker ─▶ preview switches to it
 *   release while open ─▶ preview closes and the click that follows is swallowed
 *   release / move beyond slop before holdMs ─▶ nothing (an ordinary tap or a scroll)
 *
 * The same machine serves mouse and touch; the component feeds it pointer events and
 * `stickerAt(x, y)` (a hit test against `[data-sticker-id]`).
 */
import type { CoreClock, CoreTimerHandle } from '@tg/core'

export const PREVIEW_HOLD_MS = 350
export const PREVIEW_SLOP_PX = 10

export interface PressPreviewDeps {
  clock: Pick<CoreClock, 'setTimeout' | 'clearTimeout'>
  onPreview(stickerId: string | null): void
  holdMs?: number
  slopPx?: number
}

export interface PressPreview {
  down(stickerId: string, x: number, y: number): void
  move(x: number, y: number, stickerIdUnderPointer: string | null): void
  /** Returns true when the release ended a preview, i.e. the click must be swallowed. */
  up(): boolean
  cancel(): void
  readonly open: string | null
}

export function createPressPreview(deps: PressPreviewDeps): PressPreview {
  const hold = deps.holdMs ?? PREVIEW_HOLD_MS
  const slop = deps.slopPx ?? PREVIEW_SLOP_PX
  let timer: CoreTimerHandle | null = null
  let pressed: { id: string; x: number; y: number } | null = null
  let open: string | null = null

  const clear = () => {
    if (timer !== null) deps.clock.clearTimeout(timer)
    timer = null
  }
  const setOpen = (id: string | null) => {
    if (id === open) return
    open = id
    deps.onPreview(id)
  }

  return {
    down(stickerId, x, y) {
      clear()
      pressed = { id: stickerId, x, y }
      timer = deps.clock.setTimeout(() => {
        timer = null
        if (pressed) setOpen(pressed.id)
      }, hold)
    },
    move(x, y, under) {
      if (open !== null) {
        if (under !== null) setOpen(under)
        return
      }
      if (pressed && Math.hypot(x - pressed.x, y - pressed.y) > slop) {
        clear()
        pressed = null
      }
    },
    up() {
      clear()
      pressed = null
      const wasOpen = open !== null
      setOpen(null)
      return wasOpen
    },
    cancel() {
      clear()
      pressed = null
      setOpen(null)
    },
    get open() {
      return open
    },
  }
}
