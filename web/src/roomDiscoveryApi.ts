import { authHeaders, request } from './api'
import type { Room } from './types'

export async function listDiscoverableRooms(token = ''): Promise<Room[]> {
  const response = await request('/api/rooms/discover', { headers: token ? authHeaders(token) : {} })
  if (!response.ok) throw new Error(`公开房间列表返回 ${response.status}`)
  return response.json() as Promise<Room[]>
}
