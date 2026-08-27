import { authHeaders, request } from './api'

export type RoomAiMode = 'disabled' | 'members' | 'admins'

export interface RoomAiPolicy {
  room_id: string
  mode: RoomAiMode
  version: number
  applies_to: 'new_runs_only'
  updated_at: string | null
}

async function checked(response: Response): Promise<Response> {
  if (response.status === 401) throw new Error('登录已过期')
  if (response.status === 403) throw new Error('只有房间创建者可以修改 AI 策略')
  if (response.status === 409) throw new Error('AI 策略已在其他窗口更新，请重新加载')
  if (!response.ok) throw new Error(`AI 策略请求失败：${response.status}`)
  return response
}

export async function getRoomAiPolicy(roomId: string, token: string): Promise<RoomAiPolicy> {
  const response = await request(`/api/rooms/${encodeURIComponent(roomId)}/ai-policy`, {
    cache: 'no-store',
    headers: authHeaders(token),
  })
  return checked(response).then((value) => value.json() as Promise<RoomAiPolicy>)
}

export async function updateRoomAiPolicy(
  roomId: string,
  token: string,
  mode: RoomAiMode,
  version: number,
): Promise<RoomAiPolicy> {
  const response = await request(`/api/rooms/${encodeURIComponent(roomId)}/ai-policy`, {
    method: 'PATCH',
    headers: { ...authHeaders(token), 'Content-Type': 'application/json' },
    body: JSON.stringify({ mode, version }),
  })
  return checked(response).then((value) => value.json() as Promise<RoomAiPolicy>)
}
