import axios, { AxiosError } from 'axios'
import type {
  AdminOverview,
  AiSuggestions,
  AuthSession,
  Conversation,
  FriendRequest,
  PublicConfig,
  Room,
  RoomMembership,
  SocialUser,
  StoredMessage,
  User,
} from '../types'

export const SESSION_KEY = 'qiyu.session'

export const api = axios.create({
  baseURL: '/api',
  timeout: 20_000,
})

api.interceptors.request.use((config) => {
  const raw = localStorage.getItem(SESSION_KEY)
  if (raw) {
    try {
      const session = JSON.parse(raw) as AuthSession
      config.headers.Authorization = `Bearer ${session.token}`
    } catch {
      localStorage.removeItem(SESSION_KEY)
    }
  }
  return config
})

export function errorMessage(error: unknown, fallback = '请求失败，请稍后重试') {
  if (!(error instanceof AxiosError)) return fallback
  const status = error.response?.status
  const messages: Record<number, string> = {
    400: '提交内容不符合要求',
    401: '凭据无效或已过期',
    403: '当前账号没有执行权限',
    404: '目标不存在或已被移除',
    409: '数据已发生变化，请刷新后重试',
    413: '文件超过服务器允许的大小',
    423: '聊天服务当前已锁定',
    429: '操作过于频繁，请稍后再试',
    503: '服务暂时不可用',
  }
  return status ? messages[status] ?? fallback : '无法连接到服务器'
}

export const endpoints = {
  authenticate: async (mode: 'login' | 'register', username: string, password: string) =>
    (await api.post<AuthSession>(`/users/${mode}`, { username, password })).data,
  me: async () => (await api.get<User>('/users/me')).data,
  logout: async () => api.post('/users/logout'),
  updateProfile: async (values: Partial<User>) => (await api.patch<User>('/users/me', values)).data,
  changePassword: async (current_password: string, new_password: string) =>
    api.put('/users/me/password', { current_password, new_password }),
  deleteAccount: async (current_password: string) =>
    api.delete('/users/me', { data: { current_password } }),

  config: async () => (await api.get<PublicConfig>('/config')).data,
  conversations: async () => (await api.get<Conversation[]>('/conversations')).data,
  rooms: async (name?: string) =>
    (await api.get<Room[]>('/rooms', { params: name ? { name } : undefined })).data,
  room: async (id: string) => (await api.get<Room>(`/rooms/${id}`)).data,
  createRoom: async (values: Partial<Room> & { name: string; password?: string }) =>
    (await api.post<Room>('/rooms', values)).data,
  requestJoin: async (id: string, password?: string) =>
    (await api.post<RoomMembership>(`/rooms/${id}/join-requests`, { password })).data,
  members: async (id: string) => (await api.get<RoomMembership[]>(`/rooms/${id}/members`)).data,
  invite: async (id: string, username: string) =>
    api.post(`/rooms/${id}/invitations`, { username }),
  messages: async (id: string, before?: string, password?: string) =>
    (
      await api.get<StoredMessage[]>(`/rooms/${id}/messages`, {
        params: { limit: 60, before },
        headers: password ? { 'x-room-password': password } : undefined,
      })
    ).data,
  upload: async (id: string, data: FormData, password?: string) =>
    (
      await api.post<StoredMessage>(`/rooms/${id}/attachments`, data, {
        headers: password ? { 'x-room-password': password } : undefined,
        timeout: 120_000,
      })
    ).data,
  aiSuggestions: async (id: string) =>
    (await api.post<AiSuggestions>(`/rooms/${id}/ai/suggest`)).data,

  friends: async () => (await api.get<SocialUser[]>('/friends')).data,
  requests: async (direction: 'incoming' | 'outgoing') =>
    (await api.get<FriendRequest[]>('/friend-requests', { params: { direction } })).data,
  searchUsers: async (query: string) =>
    (await api.get<SocialUser[]>('/users/search', { params: { q: query } })).data,
  addFriend: async (user_id: string) => api.post('/friend-requests', { user_id }),
  updateRequest: async (userId: string, action: 'accept' | 'reject') =>
    api.patch(`/friend-requests/${userId}`, { action: action === 'reject' ? 'decline' : action }),
  cancelRequest: async (userId: string) => api.delete(`/friend-requests/${userId}`),
  deleteFriend: async (userId: string) => api.delete(`/friends/${userId}`),
  startDirect: async (user_id: string) =>
    (await api.post<Conversation>('/direct-chats', { user_id })).data,
  blocks: async () => (await api.get<SocialUser[]>('/blocks')).data,
  block: async (userId: string) => api.put(`/blocks/${userId}`),
  unblock: async (userId: string) => api.delete(`/blocks/${userId}`),

  adminOverview: async () => (await api.get<AdminOverview>('/admin/overview')).data,
  setChatLock: async (locked: boolean) =>
    (await api.put<{ locked: boolean }>('/admin/chat-lock', { locked })).data,
  purge: async () =>
    (
      await api.post<{
        attachment_objects_deleted: number
        attachment_bytes_deleted: number
        rooms_deleted: number
      }>('/admin/maintenance/purge')
    ).data,
}

export function websocketUrl(path: string) {
  const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
  return `${protocol}//${window.location.host}${path}`
}
