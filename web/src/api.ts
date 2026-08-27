import type {
  AiSuggestions,
  BroadcastMessage,
  ChatFilePage,
  ForwardResult,
  PublicConfig,
  Room,
  RoomMembership,
  StoredMessage,
  UpdateProfilePayload,
  User,
} from './types'

export const DEFAULT_MAX_UPLOAD_BYTES = 512 * 1024 * 1024

export function formatUploadLimit(bytes: number): string {
  const mib = bytes / (1024 * 1024)
  return mib >= 1024 ? `${(mib / 1024).toFixed(mib % 1024 ? 1 : 0)} GiB` : `${mib.toFixed(mib % 1 ? 1 : 0)} MiB`
}

export async function request(path: string, options: RequestInit = {}): Promise<Response> {
  return fetch(path, {
    cache: 'no-store',
    ...options,
    headers: {
      Accept: 'application/json',
      ...options.headers,
    },
  })
}

export function authHeaders(token: string): Record<string, string> {
  return { Authorization: `Bearer ${token}` }
}

export async function getPublicConfig(): Promise<PublicConfig> {
  const response = await request('/api/config')
  if (!response.ok) throw new Error(`读取服务配置失败：${response.status}`)
  return response.json() as Promise<PublicConfig>
}

export async function getCurrentUser(token: string): Promise<User> {
  const response = await request('/api/users/me', { headers: authHeaders(token) })
  if (response.status === 401) throw new Error('登录已过期')
  if (!response.ok) throw new Error(`读取账户失败：${response.status}`)
  return response.json() as Promise<User>
}

export async function updateCurrentUser(token: string, payload: UpdateProfilePayload): Promise<User> {
  const response = await request('/api/users/me', {
    method: 'PATCH',
    headers: {
      ...authHeaders(token),
      'Content-Type': 'application/json',
    },
    body: JSON.stringify(payload),
  })
  if (response.status === 400) throw new Error('个人资料格式无效，请检查主页地址')
  if (response.status === 401) throw new Error('登录已过期')
  if (!response.ok) throw new Error(`保存头像失败：${response.status}`)
  return response.json() as Promise<User>
}

export async function uploadCurrentUserAvatar(token: string, file: File): Promise<User> {
  const body = new FormData()
  body.append('file', file)
  const response = await request('/api/users/me/avatar', { method: 'POST', headers: authHeaders(token), body })
  if (response.status === 413) throw new Error('头像不能超过 5 MiB')
  if (response.status === 415) throw new Error('请选择 PNG、JPEG、GIF、WebP 或 AVIF 图片')
  if (response.status === 401) throw new Error('登录已过期')
  if (!response.ok) throw new Error(`上传头像失败：${response.status}`)
  return response.json() as Promise<User>
}

export async function changeAccountPassword(
  token: string,
  currentPassword: string,
  newPassword: string,
): Promise<void> {
  const response = await request('/api/users/me/password', {
    method: 'PUT',
    headers: { ...authHeaders(token), 'Content-Type': 'application/json' },
    body: JSON.stringify({ current_password: currentPassword, new_password: newPassword }),
  })
  if (response.status === 400) throw new Error('新密码至少需要 8 个字符')
  if (response.status === 401) throw new Error('当前账户密码不正确')
  if (!response.ok) throw new Error(`修改密码失败：${response.status}`)
}

export async function verifyCurrentPassword(token: string, currentPassword: string): Promise<void> {
  const response = await request('/api/users/me/verify-password', {
    method: 'POST',
    headers: { ...authHeaders(token), 'Content-Type': 'application/json' },
    body: JSON.stringify({ current_password: currentPassword }),
  })
  if (response.status === 400) throw new Error('账户密码格式无效')
  if (response.status === 401) throw new Error('账户密码错误或登录已过期')
  if (!response.ok) throw new Error(`解锁验证失败：${response.status}`)
}

export async function deleteAccount(token: string, currentPassword: string): Promise<void> {
  const response = await request('/api/users/me', {
    method: 'DELETE',
    headers: { ...authHeaders(token), 'Content-Type': 'application/json' },
    body: JSON.stringify({ current_password: currentPassword }),
  })
  if (response.status === 401) throw new Error('当前账户密码不正确')
  if (!response.ok) throw new Error(`注销账户失败：${response.status}`)
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

export async function getRoom(roomId: string, token = ''): Promise<Room> {
  const response = await request(`/api/rooms/${encodeURIComponent(roomId)}`, {
    headers: token ? authHeaders(token) : {},
  })
  if (response.status === 404) throw new Error('没有找到这个聊天室，请检查 ID')
  if (!response.ok) throw new Error(`查找聊天室失败：${response.status}`)
  return response.json() as Promise<Room>
}

export async function createRoom(
  name: string,
  password: string,
  token: string,
  joinPolicy: 'open' | 'approval',
  avatarEmoji = '',
  description = '',
): Promise<Room> {
  const response = await request('/api/rooms', {
    method: 'POST',
    headers: { ...authHeaders(token), 'Content-Type': 'application/json' },
    body: JSON.stringify({
      name,
      password: password || null,
      join_policy: joinPolicy,
      avatar_emoji: avatarEmoji,
      description,
    }),
  })
  if (response.status === 400) throw new Error('房间名称或密码不符合要求')
  if (response.status === 423) throw new Error('系统已锁定聊天室，暂时无法新建')
  if (response.status === 409) throw new Error('房间名称已存在')
  if (!response.ok) throw new Error(`创建房间失败：${response.status}`)
  return response.json() as Promise<Room>
}

export async function updateRoom(
  roomId: string,
  payload: {
    name?: string
    current_password?: string
    new_password?: string
    join_policy?: 'open' | 'approval'
    avatar_emoji?: string
    description?: string
  },
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
  isSensitive = false,
): Promise<BroadcastMessage> {
  if (file.size > maxUploadBytes) throw new Error(`单个文件不能超过 ${formatUploadLimit(maxUploadBytes)}`)
  const body = new FormData()
  if (content) body.append('content', content)
  if (replyTo) body.append('reply_to', replyTo)
  if (isSensitive) body.append('is_sensitive', 'true')
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
  return storedMessageToBroadcast((await response.json()) as StoredMessage)
}

export async function listRoomFiles(
  roomId: string,
  token: string,
  password: string,
  kind: 'all' | 'image' | 'video' | 'file',
  before = '',
  limit = 50,
): Promise<ChatFilePage> {
  const query = new URLSearchParams({ kind, limit: String(limit) })
  if (before) query.set('before', before)
  const headers: Record<string, string> = authHeaders(token)
  if (password) headers['x-room-password'] = password
  const response = await request(`/api/rooms/${encodeURIComponent(roomId)}/files?${query}`, {
    headers,
  })
  if (response.status === 401) throw new Error('登录已过期或房间密码错误')
  if (response.status === 403) throw new Error('你已不是该聊天室成员')
  if (response.status === 404) throw new Error('聊天室不存在')
  if (!response.ok) throw new Error(`读取聊天文件失败：${response.status}`)
  return response.json() as Promise<ChatFilePage>
}

export async function requestRoomJoin(roomId: string, token: string, password: string): Promise<RoomMembership> {
  const response = await request(`/api/rooms/${encodeURIComponent(roomId)}/join-requests`, {
    method: 'POST',
    headers: { ...authHeaders(token), 'Content-Type': 'application/json' },
    body: JSON.stringify({ password: password || null }),
  })
  if (response.status === 401) throw new Error('房间密码错误或登录已过期')
  if (response.status === 423) throw new Error('系统已锁定聊天室，解锁后才能进入')
  if (!response.ok) throw new Error(`加入申请失败：${response.status}`)
  return response.json() as Promise<RoomMembership>
}

export async function leaveRoom(roomId: string, token: string): Promise<void> {
  const response = await request(`/api/rooms/${encodeURIComponent(roomId)}/members/me`, {
    method: 'DELETE',
    headers: authHeaders(token),
  })
  if (response.status === 409) throw new Error('聊天室内没有其他成员，暂时无法转让并退出')
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

export async function inviteRoomMember(roomId: string, token: string, username: string): Promise<RoomMembership> {
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

export async function getUserProfile(userId: string, token: string): Promise<User> {
  const response = await request(`/api/users/${encodeURIComponent(userId)}`, {
    headers: authHeaders(token),
  })
  if (response.status === 404) throw new Error('用户不存在')
  if (!response.ok) throw new Error(`读取用户资料失败：${response.status}`)
  return response.json() as Promise<User>
}

export async function forwardMessages(
  messageIds: string[],
  targetRoomIds: string[],
  token: string,
): Promise<ForwardResult[]> {
  const response = await request('/api/messages/forward', {
    method: 'POST',
    headers: { ...authHeaders(token), 'Content-Type': 'application/json' },
    body: JSON.stringify({ message_ids: messageIds, target_room_ids: targetRoomIds }),
  })
  if (!response.ok) throw new Error(`转发失败：${response.status}`)
  return response.json() as Promise<ForwardResult[]>
}

export function storedMessageToBroadcast(message: StoredMessage): BroadcastMessage {
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
    favorite_id: message.favorite_id || null,
    forwarded_from: message.forwarded_from,
    reactions: message.reactions || [],
  }
}

export async function getAiSuggestions(roomId: string, token: string, password = ''): Promise<AiSuggestions> {
  const headers = authHeaders(token)
  if (password) headers['x-room-password'] = password
  const response = await request(`/api/rooms/${encodeURIComponent(roomId)}/ai/suggest`, {
    method: 'POST',
    headers,
  })
  if (response.status === 401) throw new Error('登录已过期或聊天室密码错误')
  if (response.status === 403) throw new Error('此房间未启用 AI、仅限管理员使用，或你没有发言权限')
  if (response.status === 429) throw new Error('请求过于频繁，或 AI 并发/当日用量已达上限')
  if (response.status === 503) throw new Error('AI 模型不可用或未被部署允许')
  if (!response.ok) throw new Error(`获取 AI 建议失败：${response.status}`)
  return response.json() as Promise<AiSuggestions>
}

export async function setRoomNickname(roomId: string, token: string, nickname: string): Promise<RoomMembership> {
  const response = await request(`/api/rooms/${encodeURIComponent(roomId)}/members/me`, {
    method: 'PATCH',
    headers: { ...authHeaders(token), 'Content-Type': 'application/json' },
    body: JSON.stringify({ nickname }),
  })
  if (response.status === 400) throw new Error('昵称不符合要求')
  if (response.status === 404) throw new Error('你不是该聊天室的活跃成员')
  if (!response.ok) throw new Error(`设置昵称失败：${response.status}`)
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
