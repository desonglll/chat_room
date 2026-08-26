import { authHeaders, request } from './api'
import type { StoredMessage } from './types'

function roomHeaders(token: string, password: string): Record<string, string> {
  const headers: Record<string, string> = authHeaders(token)
  if (password) headers['x-room-password'] = password
  return headers
}

function checkRoomAccess(response: Response): void {
  if (response.status === 401) throw new Error('登录已过期或房间密码错误')
  if (response.status === 403) throw new Error('你已不是该聊天室成员')
}

export async function listRoomMessages(
  roomId: string,
  token: string,
  password: string,
  before: string,
  limit = 50,
): Promise<StoredMessage[]> {
  const query = new URLSearchParams({ limit: String(limit) })
  if (before) query.set('before', before)
  const response = await request(`/api/rooms/${encodeURIComponent(roomId)}/messages?${query}`, {
    headers: roomHeaders(token, password),
  })
  checkRoomAccess(response)
  if (!response.ok) throw new Error(`读取历史消息失败：${response.status}`)
  return response.json() as Promise<StoredMessage[]>
}

export async function listRoomMessageContext(
  roomId: string,
  messageId: string,
  token: string,
  password: string,
  limit = 60,
): Promise<StoredMessage[]> {
  const response = await request(
    `/api/rooms/${encodeURIComponent(roomId)}/messages/${encodeURIComponent(messageId)}/context?limit=${limit}`,
    { headers: roomHeaders(token, password) },
  )
  checkRoomAccess(response)
  if (response.status === 404) return []
  if (!response.ok) throw new Error(`读取消息上下文失败：${response.status}`)
  return response.json() as Promise<StoredMessage[]>
}

export async function searchRoomMessages(
  roomId: string,
  query: string,
  token: string,
  password = '',
  before = '',
  limit = 50,
): Promise<StoredMessage[]> {
  const params = new URLSearchParams({ q: query, limit: String(limit) })
  if (before) params.set('before', before)
  const response = await request(`/api/rooms/${encodeURIComponent(roomId)}/messages/search?${params}`, {
    headers: roomHeaders(token, password),
  })
  checkRoomAccess(response)
  if (!response.ok) throw new Error(`搜索消息失败：${response.status}`)
  return response.json() as Promise<StoredMessage[]>
}
