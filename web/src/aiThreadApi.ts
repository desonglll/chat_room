import { assertAiResponse, createSseParser } from './assistantApi'
import type { AiThread, AiThreadMessage } from './types'

export interface AiThreadStreamMeta {
  thread_id: string
  title: string
  room_id: string | null
  context_message_count: number
  context_format: string | null
}

export interface UpdateAiThreadPayload {
  title?: string
  room_id?: string
  clear_room?: boolean
  thinking_enabled?: boolean
}

function headers(token: string, password = ''): Record<string, string> {
  const values: Record<string, string> = {
    Authorization: `Bearer ${token}`,
    'Content-Type': 'application/json',
  }
  if (password) values['x-room-password'] = password
  return values
}

export async function listAiThreads(token: string): Promise<AiThread[]> {
  const response = await fetch('/api/ai/threads', { headers: headers(token) })
  assertAiResponse(response)
  return response.json() as Promise<AiThread[]>
}

export async function createAiThread(token: string): Promise<AiThread> {
  const response = await fetch('/api/ai/threads', {
    method: 'POST',
    headers: headers(token),
    body: '{}',
  })
  assertAiResponse(response)
  return response.json() as Promise<AiThread>
}

export async function updateAiThread(
  token: string,
  threadId: string,
  payload: UpdateAiThreadPayload,
): Promise<AiThread> {
  const response = await fetch(`/api/ai/threads/${encodeURIComponent(threadId)}`, {
    method: 'PATCH',
    headers: headers(token),
    body: JSON.stringify(payload),
  })
  assertAiResponse(response)
  return response.json() as Promise<AiThread>
}

export async function deleteAiThread(token: string, threadId: string): Promise<void> {
  const response = await fetch(`/api/ai/threads/${encodeURIComponent(threadId)}`, {
    method: 'DELETE',
    headers: headers(token),
  })
  assertAiResponse(response)
}

export async function listAiThreadMessages(token: string, threadId: string): Promise<AiThreadMessage[]> {
  const response = await fetch(`/api/ai/threads/${encodeURIComponent(threadId)}/messages`, {
    headers: headers(token),
  })
  assertAiResponse(response)
  return response.json() as Promise<AiThreadMessage[]>
}

export async function streamAiThread(
  token: string,
  threadId: string,
  question: string,
  roomId: string | null,
  password: string,
  onDelta: (content: string) => void,
  onStatus: (phase: string) => void,
): Promise<AiThreadStreamMeta> {
  const response = await fetch(`/api/ai/threads/${encodeURIComponent(threadId)}/query/stream`, {
    method: 'POST',
    headers: headers(token, password),
    body: JSON.stringify({ question, room_id: roomId }),
  })
  assertAiResponse(response)
  if (!response.body) throw new Error('当前浏览器无法接收 AI 流式响应')

  let meta: AiThreadStreamMeta | null = null
  let completed = false
  let streamError = ''
  const parser = createSseParser(({ event, data }) => {
    if (event === 'meta') meta = parseData<AiThreadStreamMeta>(data)
    if (event === 'status') onStatus(parseData<{ phase: string }>(data).phase)
    if (event === 'delta') onDelta(parseData<{ content: string }>(data).content)
    if (event === 'done') completed = true
    if (event === 'error') streamError = parseData<{ message: string }>(data).message
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

function parseData<T>(data: string): T {
  try {
    return JSON.parse(data) as T
  } catch {
    throw new Error('AI 流式响应格式无效')
  }
}
