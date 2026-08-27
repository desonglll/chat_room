import { onBeforeUnmount, ref, watch } from 'vue'
import { useRoute, useRouter, type LocationQuery, type LocationQueryRaw } from 'vue-router'
import {
  searchGlobalMessages,
  type GlobalSearchContentType,
  type GlobalSearchFilters,
  type GlobalSearchResult,
} from '../globalSearchApi'

const contentTypes = new Set<GlobalSearchContentType>(['all', 'text', 'file', 'image', 'video', 'audio'])
const emptyFilters = (): GlobalSearchFilters => ({
  q: '',
  roomId: '',
  senderId: '',
  from: '',
  to: '',
  contentType: 'all',
})

function valueOf(value: unknown): string {
  return typeof value === 'string' ? value : Array.isArray(value) && typeof value[0] === 'string' ? value[0] : ''
}

function dateValue(value: unknown): string {
  const date = valueOf(value)
  return /^\d{4}-\d{2}-\d{2}$/.test(date) && !Number.isNaN(Date.parse(`${date}T00:00:00Z`)) ? date : ''
}

export function readGlobalSearchFilters(query: LocationQuery | Record<string, unknown>): GlobalSearchFilters {
  const contentType = valueOf(query.type) as GlobalSearchContentType
  return {
    q: valueOf(query.q).trim().slice(0, 200),
    roomId: valueOf(query.room),
    senderId: valueOf(query.sender),
    from: dateValue(query.from),
    to: dateValue(query.to),
    contentType: contentTypes.has(contentType) ? contentType : 'all',
  }
}

export function globalSearchRouteQuery(filters: GlobalSearchFilters): LocationQueryRaw {
  const query: LocationQueryRaw = {}
  if (filters.q.trim()) query.q = filters.q.trim()
  if (filters.roomId) query.room = filters.roomId
  if (filters.senderId) query.sender = filters.senderId
  if (filters.from) query.from = filters.from
  if (filters.to) query.to = filters.to
  if (filters.contentType !== 'all') query.type = filters.contentType
  return query
}

export function useGlobalSearch(token: () => string) {
  const route = useRoute()
  const router = useRouter()
  const filters = ref<GlobalSearchFilters>(emptyFilters())
  const items = ref<GlobalSearchResult[]>([])
  const nextCursor = ref('')
  const loading = ref(false)
  const loadingMore = ref(false)
  const searched = ref(false)
  const error = ref('')
  let requestVersion = 0
  let controller: AbortController | null = null

  async function execute(next: GlobalSearchFilters, append = false): Promise<void> {
    if (!next.q) {
      controller?.abort()
      requestVersion += 1
      items.value = []
      nextCursor.value = ''
      loading.value = false
      loadingMore.value = false
      searched.value = false
      error.value = ''
      return
    }
    if (append && (!nextCursor.value || loadingMore.value)) return
    if (!append) {
      controller?.abort()
      controller = new AbortController()
      items.value = []
      nextCursor.value = ''
      loading.value = true
    } else {
      loadingMore.value = true
    }
    const version = ++requestVersion
    error.value = ''
    try {
      const page = await searchGlobalMessages(token(), next, append ? nextCursor.value : '', controller?.signal)
      if (version !== requestVersion) return
      items.value = append ? [...items.value, ...page.items] : page.items
      nextCursor.value = page.next_cursor || ''
      searched.value = true
    } catch (caught) {
      if (version === requestVersion && !(caught instanceof DOMException && caught.name === 'AbortError')) {
        error.value = caught instanceof Error ? caught.message : '搜索消息失败'
        searched.value = true
      }
    } finally {
      if (version === requestVersion) {
        loading.value = false
        loadingMore.value = false
      }
    }
  }

  async function submit(): Promise<void> {
    const query = globalSearchRouteQuery(filters.value)
    const target = { name: 'search' as const, query }
    if (router.resolve(target).fullPath === route.fullPath) await execute(readGlobalSearchFilters(query))
    else await router.push(target).catch(() => {})
  }

  async function loadMore(): Promise<void> {
    await execute(filters.value, true)
  }

  watch(
    () => route.query,
    (query) => {
      filters.value = readGlobalSearchFilters(query)
      void execute(filters.value)
    },
    { immediate: true },
  )
  onBeforeUnmount(() => controller?.abort())
  return { filters, items, nextCursor, loading, loadingMore, searched, error, submit, loadMore }
}
