import { useToast } from 'primevue/usetoast'

interface FavoriteMessageUpdater {
  updateMessages: (messageIds: string[]) => Promise<{ active: boolean; count: number }>
}

export function useFavoriteMessageActions(favorites: FavoriteMessageUpdater) {
  const toast = useToast()
  return async (messageIds: string[]): Promise<void> => {
    try {
      const result = await favorites.updateMessages(messageIds)
      const summary = result.active ? (result.count === 1 ? '已收藏' : `已收藏 ${result.count} 条消息`) : '已取消收藏'
      toast.add({ severity: 'success', summary, life: 2600 })
    } catch (caught) {
      toast.add({ severity: 'error', summary: caught instanceof Error ? caught.message : '收藏失败', life: 3200 })
    }
  }
}
