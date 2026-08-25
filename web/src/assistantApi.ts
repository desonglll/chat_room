import type { AiConversationResponse, AiConversationTurn } from './types'

export interface SseEvent {
  event: string
  data: string
}

export interface AiConversationStreamMeta {
  room_id: string
  context_message_count: number
  context_format: string
}

interface SseParser {
  push(chunk: string): void
  finish(): void
}

export function createSseParser(onEvent: (event: SseEvent) => void): SseParser {
  let buffer = ''

  function dispatch(block: string): void {
    let event = 'message'
    const data: string[] = []
    for (const line of block.split('\n')) {
      if (!line || line.startsWith(':')) continue
      const separator = line.indexOf(':')
      const field = separator === -1 ? line : line.slice(0, separator)
      let value = separator === -1 ? '' : line.slice(separator + 1)
      if (value.startsWith(' ')) value = value.slice(1)
      if (field === 'event') event = value
      if (field === 'data') data.push(value)
    }
    if (data.length) onEvent({ event, data: data.join('\n') })
  }

  function drain(): void {
    let boundary = buffer.indexOf('\n\n')
    while (boundary !== -1) {
      dispatch(buffer.slice(0, boundary))
      buffer = buffer.slice(boundary + 2)
      boundary = buffer.indexOf('\n\n')
    }
  }

  return {
    push(chunk: string) {
      buffer = (buffer + chunk).replaceAll('\r\n', '\n')
      drain()
    },
    finish() {
      drain()
      if (buffer.trim()) dispatch(buffer)
      buffer = ''
    },
  }
}

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
  assertAiResponse(response)
  return response.json() as Promise<AiConversationResponse>
}

export async function streamConversation(
  roomId: string,
  question: string,
  history: AiConversationTurn[],
  token: string,
  password: string,
  onDelta: (content: string) => void,
): Promise<AiConversationStreamMeta> {
  const headers: Record<string, string> = { Authorization: `Bearer ${token}`, 'Content-Type': 'application/json' }
  if (password) headers['x-room-password'] = password
  const response = await fetch(`/api/ai/conversations/${encodeURIComponent(roomId)}/query/stream`, {
    method: 'POST',
    headers,
    body: JSON.stringify({ question, history }),
  })
  assertAiResponse(response)
  if (!response.body) throw new Error('当前浏览器无法接收 AI 流式响应')

  let meta: AiConversationStreamMeta | null = null
  let completed = false
  let streamError = ''
  const parser = createSseParser(({ event, data }) => {
    if (event === 'meta') meta = parseEventData<AiConversationStreamMeta>(data)
    if (event === 'delta') onDelta(parseEventData<{ content: string }>(data).content)
    if (event === 'done') completed = true
    if (event === 'error') streamError = parseEventData<{ message: string }>(data).message
  })
  const reader = response.body.getReader()
  const decoder = new TextDecoder()
  try {
    while (true) {
      const { done, value } = await reader.read()
      if (done) break
      parser.push(decoder.decode(value, { stream: true }))
    }
    parser.push(decoder.decode())
    parser.finish()
  } finally {
    reader.releaseLock()
  }
  if (streamError) throw new Error(streamError)
  if (!completed || !meta) throw new Error('AI 流式响应意外中断')
  return meta
}

function parseEventData<T>(data: string): T {
  try {
    return JSON.parse(data) as T
  } catch {
    throw new Error('AI 流式响应格式无效')
  }
}

export function assertAiResponse(response: Response): void {
  if (response.status === 400) throw new Error('问题内容无效或问答历史过长')
  if (response.status === 401) throw new Error('登录已过期或聊天室密码错误')
  if (response.status === 403) throw new Error('你已无法访问这个会话')
  if (response.status === 404) throw new Error('会话已不存在')
  if (response.status === 429) throw new Error('请求过于频繁，请稍后再试')
  if (response.status === 503) throw new Error('AI 助手当前不可用')
  if (!response.ok) throw new Error(`AI 请求失败：${response.status}`)
}
