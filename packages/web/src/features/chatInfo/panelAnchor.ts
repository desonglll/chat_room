/**
 * Keeps the message list still while the info panel opens or closes (acceptance: "面板开合时
 * 消息列表不重排"). On a wide screen the panel takes a grid column, the conversation column
 * narrows, bubbles re-wrap and — left alone — the rows the reader is looking at slide by the
 * re-wrapped height (measured: 63 px at 1280 px with a 480 px sidebar).
 *
 * Same technique as TG-101's prepend guard (`messageList/usePrependAnchor.ts`): pin a row
 * BEFORE the layout changes, then correct `scrollTop` from ResizeObserver callbacks — after
 * layout, before paint, after Virtuoso's own observers — until the layout settles. Two pins:
 * - reading history: the topmost visible row keeps its offset from the viewport top;
 * - at the bottom: the list stays at the bottom (Telegram keeps the newest message in view).
 * It stands down on the first user input, so it never fights a reader who starts scrolling.
 *
 * The pin must be taken before React re-renders the shell, so it is driven by a synchronous
 * `uiStore` subscription (fires inside `openPanel`/`closePanel`), not by a component effect.
 * It reaches the list through TG-101's DOM contract only: the Virtuoso scroller inside the
 * conversation column and the `data-row-key` row attribute.
 */
import type { UiState } from '@tg/core'

/** Long enough for the column change, the bubble re-measure, and a late image decode. */
export const PANEL_ANCHOR_MS = 700
const AT_BOTTOM_PX = 2

export const MESSAGE_SCROLLER_SELECTOR = '.tg-shell__main [data-virtuoso-scroller]'

interface RowPin {
  key: string
  top: number
}

function topmostRow(scroller: HTMLElement): RowPin | null {
  const viewportTop = scroller.getBoundingClientRect().top
  for (const row of scroller.querySelectorAll<HTMLElement>('[data-row-key]')) {
    const rect = row.getBoundingClientRect()
    if (rect.bottom > viewportTop + 1) return { key: row.dataset.rowKey ?? '', top: rect.top - viewportTop }
  }
  return null
}

function rowTop(scroller: HTMLElement, key: string): number | null {
  const row = scroller.querySelector<HTMLElement>(`[data-row-key="${CSS.escape(key)}"]`)
  return row ? row.getBoundingClientRect().top - scroller.getBoundingClientRect().top : null
}

/** Pin the list now and hold it for `durationMs`. Returns a function that releases early. */
export function holdMessageListAnchor(root: ParentNode = document, durationMs = PANEL_ANCHOR_MS): () => void {
  const scroller = root.querySelector<HTMLElement>(MESSAGE_SCROLLER_SELECTOR)
  if (!scroller || typeof ResizeObserver === 'undefined') return () => undefined
  const atBottom = scroller.scrollHeight - scroller.scrollTop - scroller.clientHeight <= AT_BOTTOM_PX
  const pin = atBottom ? null : topmostRow(scroller)
  if (!atBottom && !pin) return () => undefined
  const until = performance.now() + durationMs

  const restore = () => {
    if (performance.now() > until) return release()
    if (atBottom) {
      const max = scroller.scrollHeight - scroller.clientHeight
      if (scroller.scrollTop < max - 1) scroller.scrollTop = max
      return
    }
    const top = pin ? rowTop(scroller, pin.key) : null
    if (top === null || !pin) return
    const delta = top - pin.top
    if (Math.abs(delta) >= 1) scroller.scrollTop += delta
  }

  const observer = new ResizeObserver(restore)
  observer.observe(scroller)
  const list = scroller.querySelector('[data-testid="virtuoso-item-list"]') ?? scroller.firstElementChild
  if (list) observer.observe(list)
  const inputs = ['wheel', 'touchstart', 'keydown', 'pointerdown'] as const
  const timer = setTimeout(() => release(), durationMs)
  let released = false
  function release() {
    if (released) return
    released = true
    clearTimeout(timer)
    observer.disconnect()
    for (const type of inputs) scroller?.removeEventListener(type, release)
  }
  for (const type of inputs) scroller.addEventListener(type, release, { passive: true })
  return release
}

type Subscribable = { subscribe(listener: (state: UiState, previous: UiState) => void): () => void }

/** Hold the anchor on every chat-info open/close. Browser-only; a no-op without a DOM. */
export function watchPanelAnchor(
  store: Subscribable,
  hold: () => () => void = () => holdMessageListAnchor(),
): () => void {
  if (typeof document === 'undefined') return () => undefined
  let release = () => undefined as void
  const stop = store.subscribe((state, previous) => {
    if ((state.activePanel === 'chatInfo') === (previous.activePanel === 'chatInfo')) return
    release() // a quick re-toggle re-pins from where the list is now
    release = hold()
  })
  return () => {
    release()
    stop()
  }
}
