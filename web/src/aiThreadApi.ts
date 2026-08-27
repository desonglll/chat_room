import { assertAiResponse } from './assistantApi'
import { readSseJsonStream } from './aiRunStream'
import type { AiModelChoice, AiRun, AiThread, AiThreadMessage } from './types'

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

export async function listAiModels(token: string): Promise<AiModelChoice[]> {
  const response = await fetch('/api/ai/models', { headers: headers(token) })
  assertAiResponse(response)
  return response.json() as Promise<AiModelChoice[]>
}

export async function createAiThread(
  token: string,
  payload: { room_id?: string; title?: string; thinking_enabled?: boolean } = {},
): Promise<AiThread> {
  const response = await fetch('/api/ai/threads', {
    method: 'POST',
    headers: headers(token),
    body: JSON.stringify(payload),
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
  modelOptionId: string | null,
): Promise<AiRun> {
  const response = await fetch(`/api/ai/threads/${encodeURIComponent(threadId)}/runs`, {
    method: 'POST',
    headers: headers(token, password),
    body: JSON.stringify({
      question,
      room_id: roomId,
      client_request_id: clientRequestId,
      model_option_id: modelOptionId,
    }),
  })
  assertAiResponse(response)
  return response.json() as Promise<AiRun>
}

export async function createCatchUpRun(
  token: string,
  threadId: string,
  roomId: string,
  password: string,
  clientRequestId: string,
  modelOptionId: string | null,
): Promise<AiRun | null> {
  const response = await fetch(`/api/ai/threads/${encodeURIComponent(threadId)}/catch-up`, {
    method: 'POST',
    headers: headers(token, password),
    body: JSON.stringify({
      room_id: roomId,
      client_request_id: clientRequestId,
      model_option_id: modelOptionId,
    }),
  })
  if (response.status === 204) return null
  assertAiResponse(response)
  return response.json() as Promise<AiRun>
}

export async function streamAiRunMessages(
  token: string,
  runId: string,
  onMessage: (message: AiThreadMessage) => void,
  signal?: AbortSignal,
): Promise<void> {
  const response = await fetch(`/api/ai/runs/${encodeURIComponent(runId)}/events`, {
    headers: headers(token),
    signal,
  })
  assertAiResponse(response)
  await readSseJsonStream<AiThreadMessage>(response, onMessage)
}
