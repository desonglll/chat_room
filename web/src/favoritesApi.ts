import { authHeaders, request } from './api'
import type { FavoriteForwardResult, FavoriteItem } from './types'

async function favoriteRequest(path: string, token: string, options: RequestInit = {}): Promise<Response> {
  const response = await request(path, {
    ...options,
    headers: {
      ...authHeaders(token),
      ...(options.body ? { 'Content-Type': 'application/json' } : {}),
      ...options.headers,
    },
  })
  if (response.status === 401) throw new Error('登录已过期')
  return response
}

export async function listFavorites(token: string): Promise<FavoriteItem[]> {
  const response = await favoriteRequest('/api/favorites', token)
  if (!response.ok) throw new Error(`读取收藏失败：${response.status}`)
  return response.json() as Promise<FavoriteItem[]>
}

export async function createFavorite(title: string, content: string, token: string): Promise<FavoriteItem> {
  const response = await favoriteRequest('/api/favorites', token, {
    method: 'POST',
    body: JSON.stringify({ title, content }),
  })
  if (response.status === 400) throw new Error('请输入收藏标题或内容')
  if (!response.ok) throw new Error(`创建收藏失败：${response.status}`)
  return response.json() as Promise<FavoriteItem>
}

export async function favoriteMessages(messageIds: string[], token: string): Promise<FavoriteItem[]> {
  const response = await favoriteRequest('/api/favorites/messages', token, {
    method: 'POST',
    body: JSON.stringify({ message_ids: messageIds }),
  })
  if (!response.ok) throw new Error(`收藏消息失败：${response.status}`)
  return response.json() as Promise<FavoriteItem[]>
}

export async function deleteFavorite(id: string, token: string): Promise<void> {
  const response = await favoriteRequest(`/api/favorites/${encodeURIComponent(id)}`, token, { method: 'DELETE' })
  if (!response.ok && response.status !== 404) throw new Error(`删除收藏失败：${response.status}`)
}

export async function forwardFavorite(
  id: string,
  targetRoomIds: string[],
  token: string,
): Promise<FavoriteForwardResult[]> {
  const response = await favoriteRequest(`/api/favorites/${encodeURIComponent(id)}/forward`, token, {
    method: 'POST',
    body: JSON.stringify({ target_room_ids: targetRoomIds }),
  })
  if (!response.ok) throw new Error(`转发收藏失败：${response.status}`)
  return response.json() as Promise<FavoriteForwardResult[]>
}
