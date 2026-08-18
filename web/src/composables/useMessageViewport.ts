import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch, type ComputedRef, type Ref } from 'vue'
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
}

const FOLLOW_DISTANCE = 72
const VIEW_THRESHOLD = 0.35

export function useMessageViewport(options: MessageViewportOptions) {
  const unseenIds = ref<string[]>([])
  const unseenCount = computed(() => unseenIds.value.length)
  const visibleIds = new Set<string>()
  let observer: IntersectionObserver | null = null
  let lastReadId = ''
  let historyInitialized = false
  let suppressScrollUntil = 0

  const broadcastIds = computed(() => options.broadcasts.value.map((message) => message.message_id))

  function isPageVisible(): boolean {
    return options.visible() && document.visibilityState === 'visible'
  }

  function isNearBottom(): boolean {
    const list = options.list.value
    return !!list && list.scrollHeight - list.scrollTop - list.clientHeight <= FOLLOW_DISTANCE
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

  function rebuildObserver(): void {
    observer?.disconnect()
    visibleIds.clear()
    const list = options.list.value
    if (!list || !('IntersectionObserver' in window)) return
    observer = new IntersectionObserver((entries) => {
      for (const entry of entries) {
        const messageId = (entry.target as HTMLElement).dataset.messageId
        if (!messageId) continue
        if (entry.isIntersecting && entry.intersectionRatio >= VIEW_THRESHOLD) visibleIds.add(messageId)
        else visibleIds.delete(messageId)
      }
      if (performance.now() >= suppressScrollUntil) markVisibleMessages()
    }, { root: list, threshold: [0, VIEW_THRESHOLD, 0.75] })
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
    const readPosition = receipt ? broadcastIds.value.indexOf(receipt.message_id) : -1
    const firstUnread = broadcastIds.value[readPosition + 1]
    suppressScrollUntil = performance.now() + 180
    if (firstUnread) {
      list.querySelector<HTMLElement>(`[data-message-id="${firstUnread}"]`)?.scrollIntoView({ block: 'start' })
    } else {
      list.scrollTop = list.scrollHeight
    }
    rebuildObserver()
  }

  function handleScroll(): void {
    if (performance.now() < suppressScrollUntil) return
    markVisibleMessages()
  }

  async function scrollToFirstUnseen(): Promise<void> {
    const firstUnseen = unseenIds.value[0]
    if (!firstUnseen) return
    await nextTick()
    options.list.value
      ?.querySelector<HTMLElement>(`[data-message-id="${firstUnseen}"]`)
      ?.scrollIntoView({ behavior: 'smooth', block: 'center' })
  }

  watch(() => options.roomId(), () => {
    observer?.disconnect()
    visibleIds.clear()
    unseenIds.value = []
    lastReadId = ''
    historyInitialized = false
    suppressScrollUntil = performance.now() + 180
  })

  watch(() => options.historyReady(), (ready) => {
    if (ready) void initializeHistory()
  })

  watch(broadcastIds, async (nextIds, previousIds) => {
    if (!historyInitialized || !options.historyReady()) return
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
        list.scrollTo({ top: list.scrollHeight, behavior: 'smooth' })
        window.setTimeout(() => markThrough(appended.at(-1)?.message_id || ''), 140)
      }
    } else if (incomingIds.length) {
      unseenIds.value = [...new Set([...unseenIds.value, ...incomingIds])]
    }
  }, { flush: 'pre' })

  watch(() => options.visible(), (visible) => {
    if (visible) void nextTick(rebuildObserver)
  })

  onMounted(() => {
    if (options.historyReady()) void initializeHistory()
  })
  onBeforeUnmount(() => observer?.disconnect())

  return { handleScroll, scrollToFirstUnseen, unseenCount }
}
