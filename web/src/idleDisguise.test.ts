import { describe, expect, test } from 'bun:test'
import { createIdleDisguiseController, IDLE_DISGUISE_DELAY_MS, type IdleScheduler } from './idleDisguise'

function createScheduler() {
  let nextHandle = 1
  const pending = new Map<number, { callback: () => void; delayMs: number }>()
  const scheduler: IdleScheduler = {
    schedule(callback, delayMs) {
      const handle = nextHandle++
      pending.set(handle, { callback, delayMs })
      return handle
    },
    cancel(handle) {
      pending.delete(handle)
    },
  }
  return { pending, scheduler }
}

function runNext(pending: Map<number, { callback: () => void; delayMs: number }>): void {
  const [handle, task] = pending.entries().next().value || []
  if (!handle || !task) throw new Error('expected a scheduled task')
  pending.delete(handle)
  task.callback()
}

describe('idle disguise controller', () => {
  test('activates after five idle seconds and hides on the next activity', () => {
    const { pending, scheduler } = createScheduler()
    const changes: boolean[] = []
    const controller = createIdleDisguiseController((active) => changes.push(active), scheduler)

    controller.setEnabled(true)
    expect([...pending.values()].map((task) => task.delayMs)).toEqual([IDLE_DISGUISE_DELAY_MS])
    runNext(pending)
    expect(changes).toEqual([true])

    controller.activity()
    expect(changes).toEqual([true, false])
    expect(pending.size).toBe(1)
  })

  test('activity restarts the timer and disabling cancels it', () => {
    const { pending, scheduler } = createScheduler()
    const changes: boolean[] = []
    const controller = createIdleDisguiseController((active) => changes.push(active), scheduler)

    controller.setEnabled(true)
    const firstHandle = pending.keys().next().value
    controller.activity()
    expect(pending.has(firstHandle as number)).toBe(false)
    expect(pending.size).toBe(1)

    controller.setEnabled(false)
    expect(pending.size).toBe(0)
    expect(changes).toEqual([])
  })
})
