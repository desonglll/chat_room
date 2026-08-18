import type { AuthSession, BroadcastMessage, Room, StoredMessage, User } from './types'

async function request(path: string, options: RequestInit = {}): Promise<Response> {
  return fetch(path, {
    cache: 'no-store',
    ...options,
    headers: {
      Accept: 'application/json',
      ...options.headers,
    },
  })
}

function authHeaders(token: string): Record<string, string> {
  return { Authorization: `Bearer ${token}` }
}

async function authenticate(
  endpoint: 'register' | 'login',
  username: string,
  password: string,
): Promise<AuthSession> {
  const response = await request(`/api/users/${endpoint}`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ username, password }),
  })
  if (response.status === 400) throw new Error('用户名不符合要求，密码至少需要 8 个字符')
  if (response.status === 401) throw new Error('用户名或密码错误')
  if (response.status === 409) throw new Error('用户名已被注册')
  if (!response.ok) throw new Error(`${endpoint === 'register' ? '注册' : '登录'}失败：${response.status}`)
  return response.json() as Promise<AuthSession>
}

export function registerUser(username: string, password: string): Promise<AuthSession> {
  return authenticate('register', username, password)
}

export function loginUser(username: string, password: string): Promise<AuthSession> {
  return authenticate('login', username, password)
}

export async function getCurrentUser(token: string): Promise<User> {
  const response = await request('/api/users/me', { headers: authHeaders(token) })
  if (response.status === 401) throw new Error('登录已过期')
  if (!response.ok) throw new Error(`读取账户失败：${response.status}`)
  return response.json() as Promise<User>
}

export async function logoutUser(token: string): Promise<void> {
  const response = await request('/api/users/logout', {
    method: 'POST',
    headers: authHeaders(token),
  })
  if (!response.ok && response.status !== 401) throw new Error(`退出登录失败：${response.status}`)
}

export async function listRooms(): Promise<Room[]> {
  const response = await request('/api/rooms')
  if (!response.ok) throw new Error(`房间列表返回 ${response.status}`)
  return response.json() as Promise<Room[]>
}

export async function createRoom(name: string, password: string): Promise<Room> {
  const response = await request('/api/rooms', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ name, password: password || null }),
  })
  if (response.status === 400) throw new Error('房间名称或密码不符合要求')
  if (response.status === 409) throw new Error('房间名称已存在')
  if (!response.ok) throw new Error(`创建房间失败：${response.status}`)
  return response.json() as Promise<Room>
}

export async function updateRoom(
  roomId: string,
  payload: { name?: string; current_password?: string; new_password?: string },
): Promise<Room> {
  const response = await request(`/api/rooms/${encodeURIComponent(roomId)}`, {
    method: 'PATCH',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(payload),
  })
  if (response.status === 400) throw new Error('名称或新密码不符合要求')
  if (response.status === 401) throw new Error('当前房间密码错误')
  if (response.status === 404) throw new Error('聊天室不存在')
  if (response.status === 409) throw new Error('房间名称已存在，或房间刚刚被修改')
  if (!response.ok) throw new Error(`保存失败：${response.status}`)
  return response.json() as Promise<Room>
}

export async function deleteRoom(roomId: string, password: string): Promise<void> {
  const headers: Record<string, string> = {}
  if (password) headers['x-room-password'] = password
  const response = await request(`/api/rooms/${encodeURIComponent(roomId)}`, {
    method: 'DELETE',
    headers,
  })
  if (response.status === 401) throw new Error('当前房间密码错误')
  if (response.status === 409) throw new Error('聊天室刚刚被修改，请重试')
  if (response.status !== 204) throw new Error(`删除失败：${response.status}`)
}

export async function uploadAttachment(
  roomId: string,
  file: File,
  token: string,
  password: string,
): Promise<BroadcastMessage> {
  const body = new FormData()
  body.append('file', file)
  const headers: Record<string, string> = authHeaders(token)
  if (password) headers['x-room-password'] = password
  const response = await request(`/api/rooms/${encodeURIComponent(roomId)}/attachments`, {
    method: 'POST',
    headers,
    body,
  })
  if (response.status === 400) throw new Error('文件为空、名称无效或上传内容无法读取')
  if (response.status === 401) throw new Error('登录已过期或房间密码错误')
  if (response.status === 404) throw new Error('聊天室不存在')
  if (response.status === 413) throw new Error('文件不能超过 50 MiB')
  if (!response.ok) throw new Error(`上传失败：${response.status}`)
  const message = await response.json() as StoredMessage
  return {
    type: 'broadcast',
    message_id: message.id,
    sender_id: message.sender_id,
    sender: message.sender,
    content: message.content,
    attachment: message.attachment,
    timestamp: message.created_at,
  }
}
