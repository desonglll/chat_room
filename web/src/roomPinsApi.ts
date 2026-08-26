import { authHeaders, request } from './api'
import type { RoomPin } from './types'

function pinPath(roomId: string, messageId = ''): string {
  const base = `/api/rooms/${encodeURIComponent(roomId)}/pins`
  return messageId ? `${base}/${encodeURIComponent(messageId)}` : base
}

export async function listRoomPins(roomId: string, token: string): Promise<RoomPin[]> {
  const response = await request(pinPath(roomId), { headers: authHeaders(token) })
  if (!response.ok) throw new Error(`读取置顶消息失败：${response.status}`)
  return response.json() as Promise<RoomPin[]>
}

export async function pinRoomMessage(roomId: string, messageId: string, token: string): Promise<RoomPin> {
  const response = await request(pinPath(roomId, messageId), { method: 'POST', headers: authHeaders(token) })
  if (response.status === 403) throw new Error('你没有置顶消息的权限')
  if (!response.ok) throw new Error(`置顶消息失败：${response.status}`)
  return response.json() as Promise<RoomPin>
}

export async function unpinRoomMessage(roomId: string, messageId: string, token: string): Promise<void> {
  const response = await request(pinPath(roomId, messageId), { method: 'DELETE', headers: authHeaders(token) })
  if (response.status === 403) throw new Error('你没有取消置顶的权限')
  if (!response.ok) throw new Error(`取消置顶失败：${response.status}`)
}
