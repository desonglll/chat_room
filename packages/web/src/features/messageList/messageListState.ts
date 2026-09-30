/**
 * State, ports and pure row derivation of the virtual list's window (TG-101). Split from
 * `messageListController.ts` so the view and tests can derive rows without the async
 * state machine. `visibleWindow` is the single definition of "what rows are on screen".
 */
import type { BroadcastMessage, DisplayMessage } from '@tg/core'
import { broadcastIds, messageKey, withoutLive } from '@tg/core'

export const FIRST_ITEM_INDEX_BASE = 10_000_000
export const OLDER_PAGE_SIZE = 50
export const CONTEXT_WINDOW_SIZE = 100
export const HIGHLIGHT_MS = 1_600

export interface MessageListApi {
  /** Messages older than (and possibly including) `beforeId`, any order. */
  loadOlder(beforeId: string, limit: number): Promise<BroadcastMessage[]>
  /** The server context window around `messageId`; [] when the message is gone. */
  loadAround(messageId: string, limit: number): Promise<BroadcastMessage[]>
}

export interface ReturnAnchor {
  /** Row key (message id for server messages) of the topmost visible row. */
  key: string
  /** That row's top minus the viewport top, in px (≤ 0 when partly scrolled out). */
  offsetPx: number
  atBottom: boolean
}

export type ScrollIndex = number | 'LAST'

export interface ScrollLocation {
  index: ScrollIndex
  align: 'start' | 'center' | 'end'
  offset?: number
}

export interface ScrollRequest extends ScrollLocation {
  seq: number
  smooth: boolean
}

export interface MessageListState {
  mode: 'live' | 'detached'
  older: BroadcastMessage[]
  olderHasMore: boolean
  detached: BroadcastMessage[]
  detachedHasOlder: boolean
  detachedHasNewer: boolean
  loadingOlder: boolean
  loadingNewer: boolean
  jumping: boolean
  viewKey: number
  firstItemIndex: number
  /** Where a freshly mounted list starts; null = the newest row. */
  initialLocation: ScrollLocation | null
  scrollRequest: ScrollRequest | null
  highlightedId: string
  returnAnchor: ReturnAnchor | null
  /** Live timeline length when the list detached; newer live rows count as unread. */
  detachedLiveMark: number
  notice: string
}

export interface VisibleWindow {
  /** Every loaded message in order, including the hidden lead. */
  all: DisplayMessage[]
  /** 1 when `all[0]` is the hidden lead, else 0. Visible rows are `all.slice(hidden)`. */
  hidden: number
}

export interface TimerPort {
  setTimeout(handler: () => void, ms: number): unknown
  clearTimeout(handle: unknown): void
}

export interface MessageListControllerOptions {
  api: MessageListApi
  /** The chat's live store timeline (read on demand, never cached). */
  getLive(): DisplayMessage[]
  timers: TimerPort
  /**
   * Resolves once the "loading older" state has been painted. The view widens its overscan
   * while loading (`usePrependOverscan`) and that must reach the screen BEFORE the rows
   * land, so an instant (cached) page is held for one frame. Awaited in parallel with the
   * request: it adds no latency to a real network round trip. Omitted in unit tests.
   */
  afterPaint?: () => Promise<void>
  /** Called synchronously right before rows are prepended (the view pins its anchor). */
  beforePrepend?: () => void
}

export const MESSAGE_NOT_FOUND_NOTICE = '消息不存在或已被删除'
export const LOAD_FAILED_NOTICE = '加载消息失败，请稍后重试'

export const initialMessageListState = (): MessageListState => ({
  mode: 'live',
  older: [],
  olderHasMore: true,
  detached: [],
  detachedHasOlder: false,
  detachedHasNewer: false,
  loadingOlder: false,
  loadingNewer: false,
  jumping: false,
  viewKey: 0,
  firstItemIndex: FIRST_ITEM_INDEX_BASE,
  initialLocation: null,
  scrollRequest: null,
  highlightedId: '',
  returnAnchor: null,
  detachedLiveMark: 0,
  notice: '',
})

/** Rows for a state + live timeline. Pure; the view memoizes it. */
export type WindowShape = Pick<MessageListState, 'mode' | 'older' | 'olderHasMore' | 'detached' | 'detachedHasOlder'>

export function visibleWindow(state: WindowShape, live: DisplayMessage[]): VisibleWindow {
  let all: DisplayMessage[]
  let hasOlder: boolean
  if (state.mode === 'detached') {
    all = state.detached
    hasOlder = state.detachedHasOlder
  } else {
    const liveIds = state.older.length > 0 ? broadcastIds(live) : null
    const older = liveIds ? withoutLive(state.older, liveIds) : state.older
    all = older.length > 0 ? (older as DisplayMessage[]).concat(live) : live
    hasOlder = state.olderHasMore
  }
  return { all, hidden: hasOlder && all.length > 1 ? 1 : 0 }
}

export function visibleKeys(view: VisibleWindow): string[] {
  const keys: string[] = new Array(Math.max(0, view.all.length - view.hidden))
  for (let index = view.hidden; index < view.all.length; index += 1) {
    keys[index - view.hidden] = messageKey(view.all[index] as DisplayMessage)
  }
  return keys
}

/** Visible row index of a message (by server id or row key), -1 when not visible. */
export function rowIndexOf(view: VisibleWindow, messageId: string): number {
  for (let index = view.hidden; index < view.all.length; index += 1) {
    const message = view.all[index] as DisplayMessage
    if (message.type === 'broadcast' && message.message_id === messageId) return index - view.hidden
    if (messageKey(message) === messageId) return index - view.hidden
  }
  return -1
}
