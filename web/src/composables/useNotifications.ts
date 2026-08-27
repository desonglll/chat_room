import { onBeforeUnmount, onMounted, ref } from 'vue'
import {
  getNotificationUnreadCount,
  listNotifications,
  markAllNotificationsRead,
  markNotificationRead,
  NOTIFICATIONS_CHANGED_EVENT,
  type NotificationItem,
  type NotificationKind,
  type NotificationsChangedSignal,
} from '../notificationsApi'

export function mergeNotificationItems(current: NotificationItem[], next: NotificationItem[]): NotificationItem[] {
  const merged = new Map(current.map((item) => [item.id, item]))
  for (const item of next) merged.set(item.id, item)
  return [...merged.values()]
}

export function useNotifications(token: () => string) {
  const items = ref<NotificationItem[]>([])
  const kind = ref<NotificationKind | ''>('')
  const nextCursor = ref<string | null>(null)
  const unreadCount = ref(0)
  const loading = ref(false)
  const loadingMore = ref(false)
  const mutating = ref(false)
  const error = ref('')
  let abortController: AbortController | null = null
  let requestVersion = 0
  let initialized = false

  async function refreshList(append = false): Promise<void> {
    const currentToken = token()
    if (!currentToken) return
    if (append && !nextCursor.value) return
    abortController?.abort()
    const controller = new AbortController()
    abortController = controller
    const version = ++requestVersion
    if (append) loadingMore.value = true
    else loading.value = true
    error.value = ''
    try {
      const page = await listNotifications(
        currentToken,
        kind.value,
        append ? nextCursor.value || '' : '',
        controller.signal,
      )
      if (version !== requestVersion) return
      items.value = append ? mergeNotificationItems(items.value, page.items) : page.items
      nextCursor.value = page.next_cursor
      initialized = true
    } catch (caught) {
      if (caught instanceof DOMException && caught.name === 'AbortError') return
      if (version === requestVersion) error.value = caught instanceof Error ? caught.message : '无法读取通知'
    } finally {
      if (version === requestVersion) {
        loading.value = false
        loadingMore.value = false
      }
    }
  }

  async function refresh(): Promise<void> {
    const currentToken = token()
    if (!currentToken) return
    const count = getNotificationUnreadCount(currentToken)
    await refreshList()
    try {
      unreadCount.value = await count
    } catch (caught) {
      if (!error.value) error.value = caught instanceof Error ? caught.message : '无法读取未读数'
    }
  }

  async function selectKind(next: NotificationKind | ''): Promise<void> {
    if (kind.value === next && initialized) return
    kind.value = next
    nextCursor.value = null
    await refreshList()
  }

  async function markRead(item: NotificationItem): Promise<void> {
    if (item.read_at || mutating.value) return
    const previousCount = unreadCount.value
    item.read_at = new Date().toISOString()
    unreadCount.value = Math.max(0, unreadCount.value - 1)
    mutating.value = true
    try {
      await markNotificationRead(token(), item.id)
    } catch (caught) {
      item.read_at = null
      unreadCount.value = previousCount
      error.value = caught instanceof Error ? caught.message : '无法标记已读'
    } finally {
      mutating.value = false
    }
  }

  async function markAllRead(): Promise<void> {
    if (!unreadCount.value || mutating.value) return
    const previous = items.value.map((item) => item.read_at)
    const previousCount = unreadCount.value
    const readAt = new Date().toISOString()
    items.value.forEach((item) => (item.read_at = item.read_at || readAt))
    unreadCount.value = 0
    mutating.value = true
    try {
      await markAllNotificationsRead(token())
    } catch (caught) {
      items.value.forEach((item, index) => (item.read_at = previous[index] || null))
      unreadCount.value = previousCount
      error.value = caught instanceof Error ? caught.message : '无法全部标记已读'
    } finally {
      mutating.value = false
    }
  }

  function handleSignal(event: Event): void {
    const signal = (event as CustomEvent<NotificationsChangedSignal>).detail
    if (!signal || signal.type !== 'notifications_changed') return
    unreadCount.value = signal.unread_count
    if (initialized) void refreshList()
  }

  onMounted(() => {
    window.addEventListener(NOTIFICATIONS_CHANGED_EVENT, handleSignal)
    void refresh()
  })
  onBeforeUnmount(() => {
    abortController?.abort()
    window.removeEventListener(NOTIFICATIONS_CHANGED_EVENT, handleSignal)
  })

  return {
    items,
    kind,
    nextCursor,
    unreadCount,
    loading,
    loadingMore,
    mutating,
    error,
    refresh,
    selectKind,
    loadMore: () => refreshList(true),
    markRead,
    markAllRead,
  }
}
