import { ref } from 'vue'
import { storageGet, storageSet } from '../browserStorage'

const WIDTH_KEY = 'chat-room.sidebar-width'
const MIN_WIDTH = 340
const MAX_WIDTH = 460
const DEFAULT_WIDTH = 380

function clamp(value: number): number {
  return Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, value))
}

export function useSidebarWidth() {
  const stored = Number(storageGet(window.localStorage, WIDTH_KEY))
  const width = ref(Number.isFinite(stored) && stored > 0 ? clamp(stored) : DEFAULT_WIDTH)
  const resizing = ref(false)
  let startX = 0
  let startWidth = 0

  function onPointerMove(event: PointerEvent): void {
    width.value = clamp(startWidth + (event.clientX - startX))
  }

  function stopResize(): void {
    if (!resizing.value) return
    resizing.value = false
    document.removeEventListener('pointermove', onPointerMove)
    document.removeEventListener('pointerup', stopResize)
    storageSet(window.localStorage, WIDTH_KEY, String(width.value))
  }

  function startResize(event: PointerEvent): void {
    resizing.value = true
    startX = event.clientX
    startWidth = width.value
    document.addEventListener('pointermove', onPointerMove)
    document.addEventListener('pointerup', stopResize)
  }

  function resizeBy(delta: number): void {
    width.value = clamp(width.value + delta)
    storageSet(window.localStorage, WIDTH_KEY, String(width.value))
  }

  return { width, resizing, startResize, resizeBy }
}
