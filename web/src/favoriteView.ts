import type { FavoriteItem } from './types'

export type FavoriteFilter = 'all' | 'file' | 'message' | 'manual'

export function matchesFavoriteFilter(item: FavoriteItem, filter: FavoriteFilter): boolean {
  if (filter === 'all') return true
  if (filter === 'file') return Boolean(item.attachment)
  if (filter === 'manual') return item.kind === 'manual'
  return item.kind === 'message' && !item.attachment
}

export function favoriteKindLabel(item: FavoriteItem): string {
  if (item.attachment) return '文件'
  if (item.kind === 'manual') return '手动收藏'
  return '对话'
}
