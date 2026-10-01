/**
 * The virtual message list (TG-101): react-virtuoso in reverse-list mode.
 *
 * - Prepend without a jump: `firstItemIndex` drops by exactly the rows inserted
 *   (controller), and the hidden lead keeps the first visible row's layout final.
 * - Older pages prefetch when the first visible row is within PREFETCH_ROWS of the top.
 * - Follows new messages only when already at the bottom (or when the reader sent one).
 * - Jump-to-message, "back to where you were", sticky date pill, unread-counting FAB.
 *
 * Virtuoso gets `totalCount` + index-based `itemContent` rather than a `data` array, so a
 * 100k-message window costs no per-render row objects: an absolute index maps to
 * `all[index - firstItemIndex + hidden]`.
 */
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import type { ListRange, VirtuosoHandle } from 'react-virtuoso'
import { Virtuoso } from 'react-virtuoso'
import type { DisplayMessage } from '@tg/core'
import {
  chatListStore,
  countUnreadBelow,
  localDayKey,
  messageKey,
  messageStore,
  selectChatById,
  selectTimeline,
} from '@tg/core'
import { useStore } from 'zustand/react'
import { Spinner, usePrefersReducedMotion } from '@tg/ui'
import { dayLabel } from './dayLabel'
import { renderDefaultMessage } from './DefaultMessage'
import { MessageListActionsContext } from './messageListActions'
import type { MessageListApi } from './messageListController'
import { FloatingDate, JumpNotice, ScrollButton, TopLoader } from './MessageListOverlays'
import { MessageRow } from './MessageRow'
import type { RenderMessage } from './renderContract'
import { usePrependAnchor } from './usePrependAnchor'
import { usePrependOverscan } from './usePrependOverscan'
import { captureAnchor, useMessageListController } from './useMessageListController'
import { t } from '../../i18n/index'

export const PREFETCH_ROWS = 20
/** Median row height of a text message; Virtuoso's estimate before measuring. */
export const ESTIMATED_ROW_HEIGHT = 56
const DATE_PILL_LINGER_MS = 1_000
const AT_BOTTOM_THRESHOLD_PX = 64
const SMOOTH_SCROLL_MAX_ROWS = 40
const EMPTY_SELECTION: ReadonlySet<string> = new Set()

export interface MessageListProps {
  chatId: string
  currentUserId: string
  renderMessage?: RenderMessage
  selectedMessageIds?: ReadonlySet<string>
  /** Deep-link target (`?message=`): jumps whenever it changes to a non-empty id. */
  targetMessageId?: string
  /** Test/benchmark injection; defaults to the chat's REST history endpoints. */
  api?: MessageListApi
}

export function MessageList({
  chatId,
  currentUserId,
  renderMessage = renderDefaultMessage,
  selectedMessageIds = EMPTY_SELECTION,
  targetMessageId = '',
  api,
}: MessageListProps) {
  const timeline = useStore(messageStore, selectTimeline(chatId))
  const chatType = useStore(chatListStore, (state) => selectChatById(chatId)(state)?.chat_type)
  const reducedMotion = usePrefersReducedMotion()
  const anchorGuard = usePrependAnchor()
  const { controller, state, view, entries } = useMessageListController({
    chatId,
    currentUserId,
    showGroupIdentity: chatType === 'group' || chatType === 'supergroup',
    live: timeline.messages,
    api,
    onBeforePrepend: anchorGuard.pin,
  })
  const virtuosoRef = useRef<VirtuosoHandle>(null)
  const scrollerRef = anchorGuard.scroller
  const rangeRef = useRef<ListRange>({ startIndex: 0, endIndex: 0 })
  const [atBottom, setAtBottom] = useState(true)
  const [topIndex, setTopIndex] = useState(-1)
  const [seenKey, setSeenKey] = useState('')
  const [scrolling, setScrolling] = useState(false)
  const lingerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const seenAbsoluteRef = useRef(-1)

  const { all, hidden } = view
  const totalCount = all.length - hidden
  const { firstItemIndex } = state
  const overscan = usePrependOverscan(state.loadingOlder)
  const at = useCallback((absolute: number) => absolute - firstItemIndex + hidden, [firstItemIndex, hidden])
  const today = localDayKey(Date.now())

  // A jump request from the controller (in-list target, return, go-to-latest).
  useEffect(() => {
    const request = state.scrollRequest
    if (!request) return
    const range = rangeRef.current
    const target = request.index === 'LAST' ? totalCount - 1 : request.index
    const distance = Math.abs(target + firstItemIndex - (range.startIndex + range.endIndex) / 2)
    const smooth = request.smooth && !reducedMotion && distance <= SMOOTH_SCROLL_MAX_ROWS
    virtuosoRef.current?.scrollToIndex({
      index: request.index,
      align: request.align,
      behavior: smooth ? 'smooth' : 'auto',
      ...(request.offset !== undefined ? { offset: request.offset } : {}),
    })
    // Only a new request (seq) scrolls; later renders must not re-apply it.
  }, [state.scrollRequest?.seq])

  // Deep links: jump once per distinct target, as soon as history has arrived.
  useEffect(() => {
    if (targetMessageId && timeline.historyReady) void controller.jumpTo(targetMessageId, null)
  }, [targetMessageId, timeline.historyReady, controller])

  // Sending while reading a detached window returns to the present, like Telegram.
  const lastLive = timeline.messages[timeline.messages.length - 1]
  const ownPending = lastLive?.type === 'broadcast' && lastLive.delivery_state === 'sending'
  useEffect(() => {
    if (ownPending && state.mode === 'detached') controller.goToLatest()
  }, [ownPending, lastLive, state.mode, controller])

  useEffect(
    () => () => {
      if (lingerRef.current !== null) clearTimeout(lingerRef.current)
    },
    [],
  )

  // Seen tracking is per mounted view: absolute indices are only stable within one viewKey.
  useEffect(() => {
    seenAbsoluteRef.current = -1
    setSeenKey('')
    setTopIndex(-1)
  }, [state.viewKey])

  // Leaving the bottom of the live list: everything down to the last row has been seen,
  // including rows Virtuoso's range report never reached. (Done in the event handler, not
  // an effect: an effect here re-renders once per replayed frame and trips React's
  // nested-update limit during a 100-frame history replay.)
  const lastMessage = all[all.length - 1]
  const handleAtBottom = useCallback(
    (value: boolean) => {
      setAtBottom(value)
      if (value || state.mode !== 'live' || !lastMessage) return
      seenAbsoluteRef.current = firstItemIndex + totalCount - 1
      setSeenKey(messageKey(lastMessage))
    },
    [state.mode, lastMessage, firstItemIndex, totalCount],
  )

  const jumpToMessage = useCallback(
    (messageId: string) => void controller.jumpTo(messageId, captureAnchor(scrollerRef.current, atBottom)),
    [controller, atBottom],
  )
  const actions = useMemo(() => ({ jumpToMessage }), [jumpToMessage])

  const handleRange = useCallback(
    (range: ListRange) => {
      rangeRef.current = range
      setTopIndex(range.startIndex)
      // Older pages only after the socket's history replay: its cursor must be final.
      if (timeline.historyReady && range.startIndex - firstItemIndex < PREFETCH_ROWS) void controller.loadOlder()
      if (state.mode === 'detached' && firstItemIndex + totalCount - 1 - range.endIndex < PREFETCH_ROWS) {
        void controller.loadNewer()
      }
      // Absolute indices survive prepends, so "furthest row seen" is a plain max.
      const bottomMessage = all[at(range.endIndex)]
      if (!bottomMessage || range.endIndex <= seenAbsoluteRef.current) return
      seenAbsoluteRef.current = range.endIndex
      setSeenKey(messageKey(bottomMessage))
    },
    [all, at, controller, firstItemIndex, state.mode, totalCount, timeline.historyReady],
  )

  const handleScrolling = useCallback((active: boolean) => {
    if (lingerRef.current !== null) clearTimeout(lingerRef.current)
    if (active) setScrolling(true)
    else lingerRef.current = setTimeout(() => setScrolling(false), DATE_PILL_LINGER_MS)
  }, [])

  const followOutput = useCallback(
    (isAtBottom: boolean) => {
      if (state.mode !== 'live') return false
      const last = timeline.messages[timeline.messages.length - 1]
      // Only a live arrival animates; replayed/caught-up rows (motion 'none') come in bursts
      // that a smooth scroll cannot keep up with, which would silently drop the follow.
      const motion = last && last.type !== 'upload' ? last.motion : undefined
      const animated = !reducedMotion && motion !== undefined && motion !== 'none'
      const smooth = animated ? 'smooth' : 'auto'
      const ownSend =
        last?.type === 'broadcast' && last.sender_id === currentUserId && last.delivery_state === 'sending'
      if (ownSend) return isAtBottom ? smooth : 'auto'
      return isAtBottom ? smooth : false
    },
    [state.mode, reducedMotion, timeline.messages, currentUserId],
  )

  const itemContent = useCallback(
    (absolute: number) => {
      const index = at(absolute)
      const message = all[index]
      const entry = entries[index]
      // Virtuoso can render one frame from its previous range while new props propagate;
      // an index outside the current window then gets a same-sized-as-estimate spacer.
      if (!message || !entry) return <div className="tg-mlist__row" style={{ height: ESTIMATED_ROW_HEIGHT }} />
      const id = message.type === 'broadcast' ? message.message_id : ''
      return (
        <MessageRow
          message={message}
          entry={entry}
          dayText={entry.startsDay ? dayLabel(entry.day, today) : ''}
          highlighted={id !== '' && id === state.highlightedId}
          selected={id !== '' && selectedMessageIds.has(id)}
          renderMessage={renderMessage}
        />
      )
    },
    [all, at, entries, today, state.highlightedId, selectedMessageIds, renderMessage],
  )

  const computeItemKey = useCallback(
    (absolute: number) => entries[at(absolute)]?.key ?? `row-${absolute}`,
    [entries, at],
  )

  const unread =
    state.mode === 'detached'
      ? countIncoming(timeline.messages.slice(state.detachedLiveMark), currentUserId)
      : atBottom
        ? 0
        : countUnreadBelow(all, seenKey, currentUserId, messageKey)
  const fabVisible = state.returnAnchor !== null || state.mode === 'detached' || !atBottom
  const topEntry = entries[at(topIndex)]

  // Mount only once the socket's history replay is complete: mounting on its first row and
  // following a 100-row burst is how a list opens short of the bottom.
  if (!timeline.historyReady && state.mode === 'live') {
    return (
      <div className="tg-mlist tg-mlist--state">
        <Spinner label={t('w.messageList.b298c8')} />
      </div>
    )
  }
  if (totalCount === 0) {
    return (
      <div className="tg-mlist tg-mlist--state">
        <p className="tg-messages__empty-pill">{t('w.messageList.7bd220')}</p>
      </div>
    )
  }

  return (
    <MessageListActionsContext.Provider value={actions}>
      <div className="tg-mlist">
        <Virtuoso
          key={state.viewKey}
          ref={virtuosoRef}
          className="tg-mlist__scroller"
          scrollerRef={anchorGuard.attach}
          firstItemIndex={firstItemIndex}
          totalCount={totalCount}
          initialTopMostItemIndex={state.initialLocation ?? { index: 'LAST', align: 'end' }}
          defaultItemHeight={ESTIMATED_ROW_HEIGHT}
          increaseViewportBy={overscan}
          alignToBottom
          // Measure rows synchronously in the ResizeObserver callback, before paint: with the
          // default rAF deferral a prepend paints one frame displaced by the page height.
          skipAnimationFrameInResizeObserver
          followOutput={followOutput}
          atBottomThreshold={AT_BOTTOM_THRESHOLD_PX}
          atBottomStateChange={handleAtBottom}
          rangeChanged={handleRange}
          isScrolling={handleScrolling}
          startReached={() => {
            if (timeline.historyReady) void controller.loadOlder()
          }}
          endReached={() => void controller.loadNewer()}
          computeItemKey={computeItemKey}
          itemContent={itemContent}
        />
        <FloatingDate text={topEntry ? dayLabel(topEntry.day, today) : ''} visible={scrolling} />
        <TopLoader active={state.loadingOlder} />
        <JumpNotice text={state.notice} onDismiss={controller.dismissNotice} />
        <ScrollButton
          visible={fabVisible}
          returning={state.returnAnchor !== null}
          unread={unread}
          busy={state.jumping}
          onClick={() => {
            const anchor = state.returnAnchor
            if (!anchor) return controller.goToLatest()
            if (!anchor.atBottom) anchorGuard.pinAt(anchor.key, anchor.offsetPx)
            void controller.returnToAnchor()
          }}
        />
      </div>
    </MessageListActionsContext.Provider>
  )
}

function countIncoming(messages: DisplayMessage[], currentUserId: string): number {
  return messages.filter(
    (message) => message.type === 'broadcast' && message.sender_id !== currentUserId && !message.recalled_at,
  ).length
}
