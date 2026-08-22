export const IDLE_DISGUISE_DELAY_MS = 5_000

export interface IdleScheduler {
  schedule(callback: () => void, delayMs: number): number
  cancel(handle: number): void
}

const browserScheduler: IdleScheduler = {
  schedule: (callback, delayMs) => window.setTimeout(callback, delayMs),
  cancel: (handle) => window.clearTimeout(handle),
}

export function createIdleDisguiseController(
  onChange: (active: boolean) => void,
  scheduler: IdleScheduler = browserScheduler,
) {
  let enabled = false
  let active = false
  let timer: number | undefined

  function clearTimer(): void {
    if (timer === undefined) return
    scheduler.cancel(timer)
    timer = undefined
  }

  function setActive(next: boolean): void {
    if (active === next) return
    active = next
    onChange(next)
  }

  function arm(): void {
    clearTimer()
    if (!enabled) return
    timer = scheduler.schedule(() => {
      timer = undefined
      setActive(true)
    }, IDLE_DISGUISE_DELAY_MS)
  }

  function setEnabled(next: boolean): void {
    enabled = next
    if (!enabled) {
      clearTimer()
      setActive(false)
      return
    }
    setActive(false)
    arm()
  }

  function activity(): void {
    if (!enabled) return
    setActive(false)
    arm()
  }

  function stop(): void {
    enabled = false
    clearTimer()
    setActive(false)
  }

  return { activity, setEnabled, stop }
}
