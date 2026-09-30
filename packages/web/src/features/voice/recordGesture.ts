/**
 * Telegram's hold-to-record gesture as a pure state machine (no DOM, no timers).
 *
 * - Press the mic: recording starts, phase `holding`.
 * - Slide left past `CANCEL_DISTANCE`: cancelled (the recording is discarded).
 * - Slide up past `LOCK_DISTANCE`: locked — hands-free, the finger may lift.
 * - Release while holding: send. A release within `TAP_MS` of the press is a tap, which on a
 *   desktop pointer (and for keyboard users) means "record hands-free", so it locks instead.
 *
 * `move` also reports how far along each threshold the pointer is (0..1), which drives the
 * «‹ 滑动取消» hint and the rising lock.
 */

export const CANCEL_DISTANCE = 120
export const LOCK_DISTANCE = 80
export const TAP_MS = 300

export type GesturePhase = 'idle' | 'holding' | 'locked'

export interface GestureState {
  phase: GesturePhase
  originX: number
  originY: number
  pressedAt: number
  /** 0..1 towards cancel (left) and towards lock (up). */
  cancelProgress: number
  lockProgress: number
}

export type GestureOutcome = 'cancel' | 'lock' | 'send' | null

export const IDLE_GESTURE: GestureState = {
  phase: 'idle',
  originX: 0,
  originY: 0,
  pressedAt: 0,
  cancelProgress: 0,
  lockProgress: 0,
}

const clamp01 = (value: number) => Math.min(1, Math.max(0, value))

export function pressGesture(x: number, y: number, at: number): GestureState {
  return { phase: 'holding', originX: x, originY: y, pressedAt: at, cancelProgress: 0, lockProgress: 0 }
}

export function moveGesture(
  state: GestureState,
  x: number,
  y: number,
): { state: GestureState; outcome: GestureOutcome } {
  if (state.phase !== 'holding') return { state, outcome: null }
  const cancelProgress = clamp01((state.originX - x) / CANCEL_DISTANCE)
  const lockProgress = clamp01((state.originY - y) / LOCK_DISTANCE)
  if (cancelProgress >= 1) return { state: IDLE_GESTURE, outcome: 'cancel' }
  if (lockProgress >= 1) {
    return { state: { ...state, phase: 'locked', cancelProgress: 0, lockProgress: 1 }, outcome: 'lock' }
  }
  return { state: { ...state, cancelProgress, lockProgress }, outcome: null }
}

export function releaseGesture(state: GestureState, at: number): { state: GestureState; outcome: GestureOutcome } {
  if (state.phase !== 'holding') return { state, outcome: null }
  if (at - state.pressedAt < TAP_MS) {
    return { state: { ...state, phase: 'locked', cancelProgress: 0, lockProgress: 1 }, outcome: 'lock' }
  }
  return { state: IDLE_GESTURE, outcome: 'send' }
}
