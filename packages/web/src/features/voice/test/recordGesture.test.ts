// TG-401: the hold / slide-to-cancel / slide-up-to-lock gesture.
import { describe, expect, test } from 'bun:test'
import { CANCEL_DISTANCE, LOCK_DISTANCE, moveGesture, pressGesture, releaseGesture, TAP_MS } from '../recordGesture'

describe('record gesture', () => {
  test('hold then release sends', () => {
    const held = pressGesture(500, 700, 0)
    expect(releaseGesture(held, TAP_MS + 1).outcome).toBe('send')
  })

  test('a quick tap locks (hands-free) instead of sending a blip', () => {
    const { state, outcome } = releaseGesture(pressGesture(500, 700, 0), TAP_MS - 1)
    expect(outcome).toBe('lock')
    expect(state.phase).toBe('locked')
  })

  test('sliding left reports progress, then cancels past the threshold', () => {
    const held = pressGesture(500, 700, 0)
    const half = moveGesture(held, 500 - CANCEL_DISTANCE / 2, 700)
    expect(half.outcome).toBeNull()
    expect(half.state.cancelProgress).toBeCloseTo(0.5)
    const gone = moveGesture(half.state, 500 - CANCEL_DISTANCE, 700)
    expect(gone.outcome).toBe('cancel')
    expect(gone.state.phase).toBe('idle')
  })

  test('sliding up locks; a locked recording ignores moves and releases', () => {
    const held = pressGesture(500, 700, 0)
    const locked = moveGesture(held, 500, 700 - LOCK_DISTANCE)
    expect(locked.outcome).toBe('lock')
    expect(locked.state.phase).toBe('locked')
    expect(moveGesture(locked.state, 0, 700).outcome).toBeNull()
    expect(releaseGesture(locked.state, 10_000).outcome).toBeNull()
  })

  test('moving right or down does nothing', () => {
    const held = pressGesture(500, 700, 0)
    const { state, outcome } = moveGesture(held, 900, 900)
    expect(outcome).toBeNull()
    expect(state.cancelProgress).toBe(0)
    expect(state.lockProgress).toBe(0)
  })
})
