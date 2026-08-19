import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch, type ComputedRef, type Ref } from 'vue'
import { preferredScrollBehavior } from '../motionPreference'
import type { BroadcastMessage, ReadReceipt } from '../types'

interface MessageViewportOptions {
  list: Ref<HTMLElement | null>
  broadcasts: ComputedRef<BroadcastMessage[]>
  roomId: () => string
  historyReady: () => boolean
  currentUserId: () => string
  readReceipts: () => ReadReceipt[]
  visible: () => boolean
  onRead: (messageId: string) => void
  onLoadOlder?: () => void
}

const FOLLOW_DISTANCE = 72
const VIEW_THRESHOLD = 0.35
const LOAD_OLDER_DISTANCE = 160

export function useMessageViewport(options: MessageViewportOptions) {
  const unseenIds = ref<string[]>([])
  const unseenCount = computed(() => unseenIds.value.length)
  const awayFromBottom = ref(false)
  const visibleIds = new Set<string>()
  let observer: IntersectionObserver | null = null
  let lastReadId = ''
  let historyInitialized = false
  let suppressScrollUntil = 0
  let visibleReadTimer: number | undefined

  const broadcastIds = computed(() => options.broadcasts.value.map((message) => message.message_id))

  function isPageVisible(): boolean {
    return options.visible() && document.visibilityState === 'visible'
  }

  function isNearBottom(): boolean {
    const list = options.list.value
    return !!list && list.scrollHeight - list.scrollTop - list.clientHeight <= FOLLOW_DISTANCE
  }

  function updateBottomState(): void {
    awayFromBottom.value = !isNearBottom()
  }

  function markThrough(messageId: string): void {
    if (!messageId || messageId === lastReadId || !isPageVisible()) return
    const position = broadcastIds.value.indexOf(messageId)
    if (position < 0) return
    lastReadId = messageId
    unseenIds.value = unseenIds.value.filter((id) => broadcastIds.value.indexOf(id) > position)
    options.onRead(messageId)
  }

  function markVisibleMessages(): void {
    let latestIndex = -1
    for (const messageId of visibleIds) {
      latestIndex = Math.max(latestIndex, broadcastIds.value.indexOf(messageId))
    }
    if (latestIndex >= 0) markThrough(broadcastIds.value[latestIndex])
  }

  function scheduleVisibleRead(): void {
    window.clearTimeout(visibleReadTimer)
    const delay = Math.max(0, suppressScrollUntil - performance.now()) + 16
    visibleReadTimer = window.setTimeout(() => {
      visibleReadTimer = undefined
      markVisibleMessages()
    }, delay)
  }

  function rebuildObserver(): void {
    observer?.disconnect()
    visibleIds.clear()
    const list = options.list.value
    if (!list || !('IntersectionObserver' in window)) return
    observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          const messageId = (entry.target as HTMLElement).dataset.messageId
          if (!messageId) continue
          if (entry.isIntersecting && entry.intersectionRatio >= VIEW_THRESHOLD) visibleIds.add(messageId)
          else visibleIds.delete(messageId)
        }
        if (performance.now() >= suppressScrollUntil) markVisibleMessages()
        else scheduleVisibleRead()
      },
      { root: list, threshold: [0, VIEW_THRESHOLD, 0.75] },
    )
    for (const element of list.querySelectorAll<HTMLElement>('[data-message-id]')) observer.observe(element)
  }

  async function initializeHistory(): Promise<void> {
    if (!options.historyReady() || historyInitialized) return
    historyInitialized = true
    await nextTick()
    const list = options.list.value
    if (!list) return
    const receipt = options.readReceipts().find((item) => item.user_id === options.currentUserId())
    lastReadId = receipt?.message_id || ''
    suppressScrollUntil = performance.now() + 180
    list.scrollTop = list.scrollHeight
    updateBottomState()
    rebuildObserver()
    markThrough(broadcastIds.value.at(-1) || '')
    scheduleVisibleRead()
  }

  function handleScroll(): void {
    updateBottomState()
    if (performance.now() < suppressScrollUntil) return
    markVisibleMessages()
    if (options.onLoadOlder && (options.list.value?.scrollTop ?? Infinity) <= LOAD_OLDER_DISTANCE) {
      options.onLoadOlder()
    }
  }

  async function scrollToBottom(): Promise<void> {
    await nextTick()
    const list = options.list.value
    if (!list) return
    unseenIds.value = []
    awayFromBottom.value = false
    suppressScrollUntil = performance.now() + 220
    list.scrollTo({ top: list.scrollHeight, behavior: preferredScrollBehavior() })
    window.setTimeout(() => markThrough(broadcastIds.value.at(-1) || ''), 240)
  }

  watch(
    () => options.roomId(),
    () => {
      observer?.disconnect()
      visibleIds.clear()
      unseenIds.value = []
      awayFromBottom.value = false
      lastReadId = ''
      historyInitialized = false
      suppressScrollUntil = performance.now() + 180
    },
  )

  watch(
    () => options.historyReady(),
    (ready) => {
      if (ready) void initializeHistory()
    },
  )

  watch(
    broadcastIds,
    async (nextIds, previousIds) => {
      if (!historyInitialized || !options.historyReady()) return

      // Older history was prepended (load-older-on-scroll-up): the old list is now a
      // trailing slice of the new one. Preserve scroll position instead of following.
      const isPrepend =
        previousIds.length > 0 &&
        nextIds.length > previousIds.length &&
        previousIds.every((id, index) => id === nextIds[index + (nextIds.length - previousIds.length)])
      if (isPrepend) {
        const list = options.list.value
        const previousScrollHeight = list?.scrollHeight ?? 0
        const previousScrollTop = list?.scrollTop ?? 0
        await nextTick()
        rebuildObserver()
        if (list) list.scrollTop = previousScrollTop + (list.scrollHeight - previousScrollHeight)
        updateBottomState()
        return
      }

      const previous = new Set(previousIds)
      const appended = options.broadcasts.value.filter((message) => !previous.has(message.message_id))
      if (!appended.length) {
        await nextTick()
        rebuildObserver()
        return
      }

      const follow = isNearBottom() && isPageVisible()
      const incomingIds = appended
        .filter((message) => message.sender_id !== options.currentUserId())
        .map((message) => message.message_id)
      await nextTick()
      rebuildObserver()
      if (follow) {
        const list = options.list.value
        if (list) {
          suppressScrollUntil = performance.now() + 120
          list.scrollTo({ top: list.scrollHeight, behavior: preferredScrollBehavior() })
          awayFromBottom.value = false
          window.setTimeout(() => markThrough(appended.at(-1)?.message_id || ''), 140)
        }
      } else if (incomingIds.length) {
        unseenIds.value = [...new Set([...unseenIds.value, ...incomingIds])]
      }
    },
    { flush: 'pre' },
  )

  watch(
    () => options.visible(),
    (visible) => {
      if (visible) {
        void nextTick(() => {
          rebuildObserver()
          scheduleVisibleRead()
        })
      }
    },
  )

  function handleVisibilityChange(): void {
    if (document.visibilityState !== 'visible' || !options.visible()) return
    void nextTick(() => {
      rebuildObserver()
      scheduleVisibleRead()
    })
  }

  onMounted(() => {
    document.addEventListener('visibilitychange', handleVisibilityChange)
    if (options.historyReady()) void initializeHistory()
  })
  onBeforeUnmount(() => {
    observer?.disconnect()
    window.clearTimeout(visibleReadTimer)
    document.removeEventListener('visibilitychange', handleVisibilityChange)
  })

  return { awayFromBottom, handleScroll, scrollToBottom, unseenCount }
}
