import type { AiConversationResponse, AiConversationTurn } from './types'

export async function queryConversation(
  roomId: string,
  question: string,
  history: AiConversationTurn[],
  token: string,
  password = '',
): Promise<AiConversationResponse> {
  const headers: Record<string, string> = { Authorization: `Bearer ${token}`, 'Content-Type': 'application/json' }
  if (password) headers['x-room-password'] = password
  const response = await fetch(`/api/ai/conversations/${encodeURIComponent(roomId)}/query`, {
    method: 'POST',
    headers,
    body: JSON.stringify({ question, history }),
  })
  if (response.status === 400) throw new Error('问题内容无效或问答历史过长')
  if (response.status === 401) throw new Error('登录已过期或聊天室密码错误')
  if (response.status === 403) throw new Error('你已无法访问这个会话')
  if (response.status === 404) throw new Error('会话已不存在')
  if (response.status === 429) throw new Error('请求过于频繁，请稍后再试')
  if (response.status === 503) throw new Error('AI 助手当前不可用')
  if (!response.ok) throw new Error(`AI 请求失败：${response.status}`)
  return response.json() as Promise<AiConversationResponse>
}
