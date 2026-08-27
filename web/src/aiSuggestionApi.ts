import { readSseJsonStream } from './aiRunStream'
import { authHeaders } from './api'

export interface AiSuggestionStreamItem {
  type: 'suggestion' | 'summary'
  content: string
}

function parseLine(line: string): AiSuggestionStreamItem | null {
  const trimmed = line.trim()
  if (!trimmed || trimmed.startsWith('```')) return null
  const start = trimmed.indexOf('{')
  const end = trimmed.lastIndexOf('}')
  if (start === -1 || end < start) return null
  let value: { type?: string; content?: unknown }
  try {
    value = JSON.parse(trimmed.slice(start, end + 1)) as { type?: string; content?: unknown }
  } catch {
    return null
  }
  if (value.type === 'error') throw new Error(typeof value.content === 'string' ? value.content : 'AI 助手当前不可用')
  if ((value.type !== 'suggestion' && value.type !== 'summary') || typeof value.content !== 'string') return null
  const content = value.content.trim()
  return content ? { type: value.type, content } : null
}

export async function streamAiSuggestions(
  roomId: string,
  token: string,
  password: string,
  onItem: (item: AiSuggestionStreamItem) => void,
  signal?: AbortSignal,
): Promise<void> {
  const headers: Record<string, string> = { ...authHeaders(token), Accept: 'text/event-stream' }
  if (password) headers['x-room-password'] = password
  const response = await fetch(`/api/rooms/${encodeURIComponent(roomId)}/ai/suggest/events`, {
    method: 'POST',
    headers,
    signal,
  })
  if (response.status === 401) throw new Error('登录已过期或聊天室密码错误')
  if (response.status === 403) throw new Error('此房间未启用 AI、仅限管理员使用，或你没有发言权限')
  if (response.status === 429) throw new Error('请求过于频繁，或 AI 并发/当日用量已达上限')
  if (response.status === 503) throw new Error('AI 模型不可用或未被部署允许')
  if (!response.ok) throw new Error(`获取 AI 建议失败：${response.status}`)

  let buffer = ''
  let itemCount = 0
  const consumeLines = (final = false): void => {
    const lines = buffer.split('\n')
    const tail = lines.pop() || ''
    buffer = final ? '' : tail
    if (final && tail) lines.push(tail)
    for (const line of lines) {
      const item = parseLine(line)
      if (!item) continue
      itemCount += 1
      onItem(item)
    }
  }
  await readSseJsonStream<string>(response, (chunk) => {
    buffer += chunk
    consumeLines()
  })
  consumeLines(true)
  if (!itemCount) throw new Error('AI 没有返回可用建议')
}
