import { authHeaders, request } from './api'
import type { ConversationSummary, FriendRequest, SocialUser } from './types'

async function socialRequest(path: string, token: string, options: RequestInit = {}): Promise<Response> {
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

export async function listConversations(token: string): Promise<ConversationSummary[]> {
  const response = await socialRequest('/api/conversations', token)
  if (!response.ok) throw new Error(`读取会话失败：${response.status}`)
  return response.json() as Promise<ConversationSummary[]>
}

export async function setConversationAlias(roomId: string, alias: string, token: string): Promise<ConversationSummary> {
  const response = await socialRequest(`/api/conversations/${encodeURIComponent(roomId)}/alias`, token, {
    method: 'PUT',
    body: JSON.stringify({ alias }),
  })
  if (response.status === 400) throw new Error('备注最多 64 个字符，且不能包含控制字符')
  if (response.status === 404) throw new Error('会话已失效，请刷新后重试')
  if (!response.ok) throw new Error(`保存备注失败：${response.status}`)
  return response.json() as Promise<ConversationSummary>
}

export async function searchUsers(query: string, token: string): Promise<SocialUser[]> {
  const params = new URLSearchParams({ q: query, limit: '30' })
  const response = await socialRequest(`/api/users/search?${params}`, token)
  if (response.status === 400) throw new Error('请输入至少 2 个字符')
  if (!response.ok) throw new Error(`搜索用户失败：${response.status}`)
  return response.json() as Promise<SocialUser[]>
}

export async function listFriends(token: string): Promise<SocialUser[]> {
  const response = await socialRequest('/api/friends', token)
  if (!response.ok) throw new Error(`读取好友失败：${response.status}`)
  return response.json() as Promise<SocialUser[]>
}

export async function listFriendRequests(direction: 'incoming' | 'outgoing', token: string): Promise<FriendRequest[]> {
  const response = await socialRequest(`/api/friend-requests?direction=${direction}`, token)
  if (!response.ok) throw new Error(`读取好友申请失败：${response.status}`)
  return response.json() as Promise<FriendRequest[]>
}

export async function sendFriendRequest(userId: string, token: string): Promise<void> {
  const response = await socialRequest('/api/friend-requests', token, {
    method: 'POST',
    body: JSON.stringify({ user_id: userId }),
  })
  if (response.status === 404) throw new Error('该用户当前不可添加')
  if (!response.ok) throw new Error(`发送好友申请失败：${response.status}`)
}

export async function respondFriendRequest(userId: string, action: 'accept' | 'decline', token: string): Promise<void> {
  const response = await socialRequest(`/api/friend-requests/${encodeURIComponent(userId)}`, token, {
    method: 'PATCH',
    body: JSON.stringify({ action }),
  })
  if (response.status === 404) throw new Error('好友申请已失效')
  if (!response.ok) throw new Error(`处理好友申请失败：${response.status}`)
}

export async function cancelFriendRequest(userId: string, token: string): Promise<void> {
  const response = await socialRequest(`/api/friend-requests/${encodeURIComponent(userId)}`, token, {
    method: 'DELETE',
  })
  if (!response.ok && response.status !== 404) throw new Error(`取消好友申请失败：${response.status}`)
}

export async function removeFriend(userId: string, token: string): Promise<void> {
  const response = await socialRequest(`/api/friends/${encodeURIComponent(userId)}`, token, { method: 'DELETE' })
  if (!response.ok) throw new Error(`删除好友失败：${response.status}`)
}

export async function setFriendRemark(userId: string, remark: string, token: string): Promise<void> {
  const response = await socialRequest(`/api/friends/${encodeURIComponent(userId)}/remark`, token, {
    method: 'PUT',
    body: JSON.stringify({ remark }),
  })
  if (response.status === 400) throw new Error('好友备注最多 64 个字符')
  if (response.status === 404) throw new Error('好友关系已失效')
  if (!response.ok) throw new Error(`保存好友备注失败：${response.status}`)
}

export async function listBlockedUsers(token: string): Promise<SocialUser[]> {
  const response = await socialRequest('/api/blocks', token)
  if (!response.ok) throw new Error(`读取黑名单失败：${response.status}`)
  return response.json() as Promise<SocialUser[]>
}

export async function blockUser(userId: string, token: string): Promise<void> {
  const response = await socialRequest(`/api/blocks/${encodeURIComponent(userId)}`, token, { method: 'PUT' })
  if (response.status === 404) throw new Error('用户不存在')
  if (!response.ok) throw new Error(`拉黑失败：${response.status}`)
}

export async function unblockUser(userId: string, token: string): Promise<void> {
  const response = await socialRequest(`/api/blocks/${encodeURIComponent(userId)}`, token, { method: 'DELETE' })
  if (!response.ok) throw new Error(`取消拉黑失败：${response.status}`)
}

export async function startDirectChat(userId: string, token: string): Promise<ConversationSummary> {
  const response = await socialRequest('/api/direct-chats', token, {
    method: 'POST',
    body: JSON.stringify({ user_id: userId }),
  })
  if (response.status === 409) throw new Error('成为好友后才能开始私聊')
  if (response.status === 423) throw new Error('系统已锁定聊天室，解锁后才能开始私聊')
  if (!response.ok) throw new Error(`开始私聊失败：${response.status}`)
  return response.json() as Promise<ConversationSummary>
}
