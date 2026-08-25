import { authHeaders, request } from './api'
import type { FavoriteCollaborator, FavoriteForwardResult, FavoriteItem } from './types'

export class FavoriteConflictError extends Error {}

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

export async function updateFavorite(
  id: string,
  version: number,
  title: string,
  content: string,
  token: string,
): Promise<FavoriteItem> {
  const response = await favoriteRequest(`/api/favorites/${encodeURIComponent(id)}`, token, {
    method: 'PUT',
    body: JSON.stringify({ version, title, content }),
  })
  if (response.status === 409) throw new FavoriteConflictError('收藏已被其他协作者更新，已加载最新版本，请检查后再保存')
  if (response.status === 400) throw new Error('收藏标题或内容不符合要求')
  if (!response.ok) throw new Error(`更新收藏失败：${response.status}`)
  return response.json() as Promise<FavoriteItem>
}

export async function listFavoriteCollaborators(id: string, token: string): Promise<FavoriteCollaborator[]> {
  const response = await favoriteRequest(`/api/favorites/${encodeURIComponent(id)}/collaborators`, token)
  if (!response.ok) throw new Error(`读取协作者失败：${response.status}`)
  return response.json() as Promise<FavoriteCollaborator[]>
}

export async function addFavoriteCollaborator(
  id: string,
  userId: string,
  token: string,
): Promise<FavoriteCollaborator> {
  const response = await favoriteRequest(`/api/favorites/${encodeURIComponent(id)}/collaborators`, token, {
    method: 'POST',
    body: JSON.stringify({ user_id: userId }),
  })
  if (response.status === 400) throw new Error('只能邀请当前好友协作')
  if (response.status === 403) throw new Error('只有收藏所有者可以邀请协作者')
  if (!response.ok) throw new Error(`添加协作者失败：${response.status}`)
  return response.json() as Promise<FavoriteCollaborator>
}

export async function removeFavoriteCollaborator(id: string, userId: string, token: string): Promise<void> {
  const response = await favoriteRequest(
    `/api/favorites/${encodeURIComponent(id)}/collaborators/${encodeURIComponent(userId)}`,
    token,
    { method: 'DELETE' },
  )
  if (!response.ok) throw new Error(`移除协作者失败：${response.status}`)
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
