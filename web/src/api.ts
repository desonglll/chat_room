import type { AuthSession, BroadcastMessage, PublicConfig, Room, RoomMembership, StoredMessage, User } from './types'

export const DEFAULT_MAX_UPLOAD_BYTES = 512 * 1024 * 1024

export function formatUploadLimit(bytes: number): string {
  const mib = bytes / (1024 * 1024)
  return mib >= 1024 ? `${(mib / 1024).toFixed(mib % 1024 ? 1 : 0)} GiB` : `${mib.toFixed(mib % 1 ? 1 : 0)} MiB`
}

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

export async function getPublicConfig(): Promise<PublicConfig> {
  const response = await request('/api/config')
  if (!response.ok) throw new Error(`读取服务配置失败：${response.status}`)
  return response.json() as Promise<PublicConfig>
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

export async function updateCurrentUser(token: string, avatarEmoji: string): Promise<User> {
  const response = await request('/api/users/me', {
    method: 'PATCH',
    headers: {
      ...authHeaders(token),
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({ avatar_emoji: avatarEmoji }),
  })
  if (response.status === 400) throw new Error('头像格式无效')
  if (response.status === 401) throw new Error('登录已过期')
  if (!response.ok) throw new Error(`保存头像失败：${response.status}`)
  return response.json() as Promise<User>
}

export async function logoutUser(token: string): Promise<void> {
  const response = await request('/api/users/logout', {
    method: 'POST',
    headers: authHeaders(token),
  })
  if (!response.ok && response.status !== 401) throw new Error(`退出登录失败：${response.status}`)
}

export async function listRooms(token = ''): Promise<Room[]> {
  const response = await request('/api/rooms', { headers: token ? authHeaders(token) : {} })
  if (!response.ok) throw new Error(`房间列表返回 ${response.status}`)
  return response.json() as Promise<Room[]>
}

export async function createRoom(
  name: string,
  password: string,
  token: string,
  joinPolicy: 'open' | 'approval',
): Promise<Room> {
  const response = await request('/api/rooms', {
    method: 'POST',
    headers: { ...authHeaders(token), 'Content-Type': 'application/json' },
    body: JSON.stringify({ name, password: password || null, join_policy: joinPolicy }),
  })
  if (response.status === 400) throw new Error('房间名称或密码不符合要求')
  if (response.status === 409) throw new Error('房间名称已存在')
  if (!response.ok) throw new Error(`创建房间失败：${response.status}`)
  return response.json() as Promise<Room>
}

export async function updateRoom(
  roomId: string,
  payload: { name?: string; current_password?: string; new_password?: string; join_policy?: 'open' | 'approval' },
  token: string,
): Promise<Room> {
  const response = await request(`/api/rooms/${encodeURIComponent(roomId)}`, {
    method: 'PATCH',
    headers: { ...authHeaders(token), 'Content-Type': 'application/json' },
    body: JSON.stringify(payload),
  })
  if (response.status === 400) throw new Error('名称或新密码不符合要求')
  if (response.status === 401) throw new Error('当前房间密码错误')
  if (response.status === 403) throw new Error('你没有管理聊天室的权限')
  if (response.status === 404) throw new Error('聊天室不存在')
  if (response.status === 409) throw new Error('房间名称已存在，或房间刚刚被修改')
  if (!response.ok) throw new Error(`保存失败：${response.status}`)
  return response.json() as Promise<Room>
}

export async function deleteRoom(roomId: string, token: string): Promise<void> {
  const response = await request(`/api/rooms/${encodeURIComponent(roomId)}`, {
    method: 'DELETE',
    headers: authHeaders(token),
  })
  if (response.status === 401) throw new Error('当前房间密码错误')
  if (response.status === 403) throw new Error('只有房间创建者可以删除聊天室')
  if (response.status === 409) throw new Error('聊天室刚刚被修改，请重试')
  if (response.status !== 204) throw new Error(`删除失败：${response.status}`)
}

export async function uploadAttachment(
  roomId: string,
  file: File,
  token: string,
  password: string,
  content = '',
  replyTo = '',
  maxUploadBytes = DEFAULT_MAX_UPLOAD_BYTES,
): Promise<BroadcastMessage> {
  if (file.size > maxUploadBytes) throw new Error(`单个文件不能超过 ${formatUploadLimit(maxUploadBytes)}`)
  const body = new FormData()
  if (content) body.append('content', content)
  if (replyTo) body.append('reply_to', replyTo)
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
  if (response.status === 403) throw new Error('你已不是该聊天室成员')
  if (response.status === 404) throw new Error('聊天室不存在')
  if (response.status === 413) throw new Error(`文件不能超过 ${formatUploadLimit(maxUploadBytes)}`)
  if (!response.ok) throw new Error(`上传失败：${response.status}`)
  const message = await response.json() as StoredMessage
  return {
    type: 'broadcast',
    message_id: message.id,
    sender_id: message.sender_id,
    sender: message.sender,
    sender_avatar: message.sender_avatar,
    content: message.content,
    attachment: message.attachment,
    reply_to: message.reply_to,
    recalled_at: message.recalled_at,
    edited_at: message.edited_at,
    timestamp: message.created_at,
  }
}

export async function requestRoomJoin(
  roomId: string,
  token: string,
  password: string,
): Promise<RoomMembership> {
  const response = await request(`/api/rooms/${encodeURIComponent(roomId)}/join-requests`, {
    method: 'POST',
    headers: { ...authHeaders(token), 'Content-Type': 'application/json' },
    body: JSON.stringify({ password: password || null }),
  })
  if (response.status === 401) throw new Error('房间密码错误或登录已过期')
  if (!response.ok) throw new Error(`加入申请失败：${response.status}`)
  return response.json() as Promise<RoomMembership>
}

export async function leaveRoom(roomId: string, token: string): Promise<void> {
  const response = await request(`/api/rooms/${encodeURIComponent(roomId)}/members/me`, {
    method: 'DELETE',
    headers: authHeaders(token),
  })
  if (response.status === 409) throw new Error('聊天室创建者不能直接退出，可以删除聊天室')
  if (!response.ok) throw new Error(`退出聊天室失败：${response.status}`)
}

export async function listRoomMembers(roomId: string, token: string): Promise<RoomMembership[]> {
  const response = await request(`/api/rooms/${encodeURIComponent(roomId)}/members`, {
    headers: authHeaders(token),
  })
  if (response.status === 403) throw new Error('你没有管理成员的权限')
  if (!response.ok) throw new Error(`读取成员失败：${response.status}`)
  return response.json() as Promise<RoomMembership[]>
}

export async function inviteRoomMember(
  roomId: string,
  token: string,
  username: string,
): Promise<RoomMembership> {
  const response = await request(`/api/rooms/${encodeURIComponent(roomId)}/invitations`, {
    method: 'POST',
    headers: { ...authHeaders(token), 'Content-Type': 'application/json' },
    body: JSON.stringify({ username }),
  })
  if (response.status === 404) throw new Error('没有找到这个用户')
  if (response.status === 403) throw new Error('你没有邀请成员的权限')
  if (!response.ok) throw new Error(`邀请失败：${response.status}`)
  return response.json() as Promise<RoomMembership>
}

export async function updateRoomMember(
  roomId: string,
  userId: string,
  token: string,
  action: 'approve' | 'reject' | 'remove' | 'set_role',
  role?: 'admin' | 'member',
): Promise<RoomMembership> {
  const response = await request(`/api/rooms/${encodeURIComponent(roomId)}/members/${encodeURIComponent(userId)}`, {
    method: 'PATCH',
    headers: { ...authHeaders(token), 'Content-Type': 'application/json' },
    body: JSON.stringify({ action, role }),
  })
  if (response.status === 403) throw new Error('你没有执行此操作的权限')
  if (response.status === 409) throw new Error('成员状态已变化，或不能修改创建者')
  if (!response.ok) throw new Error(`成员操作失败：${response.status}`)
  return response.json() as Promise<RoomMembership>
}
