import { assertAiResponse } from './assistantApi'
import type { AiRun, AiThread, AiThreadMessage } from './types'

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

export async function createAiRun(
  token: string,
  threadId: string,
  question: string,
  roomId: string | null,
  password: string,
  clientRequestId: string,
): Promise<AiRun> {
  const response = await fetch(`/api/ai/threads/${encodeURIComponent(threadId)}/runs`, {
    method: 'POST',
    headers: headers(token, password),
    body: JSON.stringify({ question, room_id: roomId, client_request_id: clientRequestId }),
  })
  assertAiResponse(response)
  return response.json() as Promise<AiRun>
}
