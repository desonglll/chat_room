/**
 * Time → frame for one playing animation. Pure; the ticker owns wall time.
 *
 * Frames are derived from elapsed time, not counted per tick, so an animation keeps its real
 * speed when ticks are dropped (the budgeted ticker skips renders under load) and resumes where
 * it paused rather than jumping ahead (paused time is simply never fed in).
 */

export interface PlayerClock {
  /** Fractional frame position in [0, frames). */
  position: number
  readonly frames: number
  readonly frameRate: number
  readonly loop: boolean
  ended: boolean
}

/** A stall longer than this (tab switch, long task, debugger) advances by this much only. */
export const MAX_STEP_MS = 100

export function createClock(frames: number, frameRate: number, loop: boolean): PlayerClock {
  return { position: 0, frames: Math.max(1, Math.floor(frames)), frameRate, loop, ended: false }
}

export function advanceClock(clock: PlayerClock, elapsedMs: number): void {
  if (clock.ended || elapsedMs <= 0) return
  const next = clock.position + (Math.min(elapsedMs, MAX_STEP_MS) * clock.frameRate) / 1000
  if (next < clock.frames) {
    clock.position = next
  } else if (clock.loop) {
    clock.position = next % clock.frames
  } else {
    clock.position = clock.frames - 1
    clock.ended = true
  }
}

/**
 * The frame to draw at `fpsCap`. A 60 fps sticker capped at 30 draws every second frame, so
 * the cap halves render work without slowing the animation down.
 *
 * `ahead` asks for the frame one step in the future. An asynchronous renderer (the worker
 * pool) needs a round trip per frame; asking one step early hides that latency, at the cost
 * of a constant one-step phase lead nobody can see on a sticker.
 */
export function frameToDraw(clock: PlayerClock, fpsCap: number, ahead = false): number {
  const stride = Math.max(1, Math.round(clock.frameRate / fpsCap))
  if (clock.ended) return clock.frames - 1
  let frame = Math.floor(clock.position) + (ahead ? stride : 0)
  if (frame >= clock.frames) frame = clock.loop ? frame % clock.frames : clock.frames - 1
  return frame - (frame % stride)
}

export function restartClock(clock: PlayerClock): void {
  clock.position = 0
  clock.ended = false
}
