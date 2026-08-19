import { onBeforeUnmount, ref, watch, type Ref } from 'vue'

interface DelayedVisibilityOptions {
  delayMs?: number
  minimumMs?: number
}

export function useDelayedVisibility(source: Ref<boolean>, options: DelayedVisibilityOptions = {}) {
  const delayMs = options.delayMs ?? 180
  const minimumMs = options.minimumMs ?? 240
  const visible = ref(false)
  let showTimer: number | undefined
  let hideTimer: number | undefined
  let shownAt = 0

  function clearTimers(): void {
    window.clearTimeout(showTimer)
    window.clearTimeout(hideTimer)
    showTimer = undefined
    hideTimer = undefined
  }

  watch(
    source,
    (pending) => {
      clearTimers()
      if (pending) {
        showTimer = window.setTimeout(() => {
          visible.value = true
          shownAt = performance.now()
        }, delayMs)
        return
      }
      if (!visible.value) return
      const remaining = Math.max(0, minimumMs - (performance.now() - shownAt))
      hideTimer = window.setTimeout(() => {
        visible.value = false
      }, remaining)
    },
    { immediate: true },
  )

  onBeforeUnmount(clearTimers)
  return visible
}
