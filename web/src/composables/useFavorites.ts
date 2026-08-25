import { computed, ref, watch, type Ref } from 'vue'
import {
  createFavorite as createFavoriteRequest,
  deleteFavorite as deleteFavoriteRequest,
  favoriteMessages as favoriteMessagesRequest,
  forwardFavorite as forwardFavoriteRequest,
  listFavorites,
} from '../favoritesApi'
import type { FavoriteItem } from '../types'

export function useFavorites(token: Ref<string>) {
  const items = ref<FavoriteItem[]>([])
  const loading = ref(false)
  const error = ref('')
  const messageIds = computed(() =>
    items.value.flatMap((item) => (item.source_message_id ? [item.source_message_id] : [])),
  )

  async function refresh(): Promise<void> {
    const activeToken = token.value
    if (!activeToken) {
      items.value = []
      return
    }
    loading.value = true
    try {
      items.value = await listFavorites(activeToken)
      error.value = ''
    } catch (caught) {
      error.value = caught instanceof Error ? caught.message : '无法读取收藏'
    } finally {
      loading.value = false
    }
  }

  async function create(title: string, content: string): Promise<FavoriteItem> {
    const item = await createFavoriteRequest(title, content, token.value)
    items.value = [item, ...items.value]
    return item
  }

  async function addMessages(messageIds: string[]): Promise<FavoriteItem[]> {
    const added = await favoriteMessagesRequest(messageIds, token.value)
    const ids = new Set(added.map((item) => item.id))
    items.value = [...added, ...items.value.filter((item) => !ids.has(item.id))]
    return added
  }

  async function remove(id: string): Promise<void> {
    await deleteFavoriteRequest(id, token.value)
    items.value = items.value.filter((item) => item.id !== id)
  }

  async function toggleMessage(messageId: string): Promise<boolean> {
    const existing = items.value.find((item) => item.source_message_id === messageId)
    if (existing) {
      await remove(existing.id)
      return false
    }
    await addMessages([messageId])
    return true
  }

  async function updateMessages(messageIds: string[]): Promise<{ active: boolean; count: number }> {
    if (messageIds.length === 1) return { active: await toggleMessage(messageIds[0]), count: 1 }
    return { active: true, count: (await addMessages(messageIds)).length }
  }

  const forward = (id: string, targetRoomIds: string[]) => forwardFavoriteRequest(id, targetRoomIds, token.value)
  watch(token, () => void refresh(), { immediate: true })
  return {
    items,
    messageIds,
    loading,
    error,
    refresh,
    create,
    addMessages,
    toggleMessage,
    updateMessages,
    remove,
    forward,
  }
}
