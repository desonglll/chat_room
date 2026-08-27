import { authHeaders, request } from './api'

export type AiExtractionKind = 'decision' | 'task'
export type AiExtractionCandidateStatus = 'proposed' | 'confirmed' | 'dismissed'

export interface AiExtractionSource {
  message_id: string
  sender: string
  excerpt: string
  sent_at: string
}

export interface AiExtractionCandidate {
  id: string
  kind: AiExtractionKind
  title: string
  detail: string
  inferred: boolean
  sources: AiExtractionSource[]
  status: AiExtractionCandidateStatus
  result_kind: 'favorite' | 'task' | null
  result_id: string | null
  version: number
  created_at: string
  updated_at: string
}

export interface AiExtractionRun {
  id: string
  room_id: string
  client_request_id: string
  from_at: string
  to_at: string
  model_option_id: string | null
  provider: string
  model: string
  status: 'queued' | 'running' | 'completed' | 'failed'
  message_count: number | null
  error_message: string | null
  candidates: AiExtractionCandidate[]
  created_at: string
  updated_at: string
}

export class AiExtractionApiError extends Error {
  constructor(
    public readonly status: number,
    message: string,
  ) {
    super(message)
  }
}

function headers(token: string, password: string, json = false): Record<string, string> {
  const values = authHeaders(token)
  if (password) values['x-room-password'] = password
  if (json) values['Content-Type'] = 'application/json'
  return values
}

async function checked(response: Response): Promise<Response> {
  if (response.status === 400 || response.status === 422) {
    throw new AiExtractionApiError(response.status, '提取范围或候选操作无效')
  }
  if (response.status === 401) throw new AiExtractionApiError(401, '聊天室密码错误或登录已过期')
  if (response.status === 403) throw new AiExtractionApiError(403, '此房间未启用 AI、仅限管理员使用，或你已无权访问')
  if (response.status === 404) throw new AiExtractionApiError(404, '提取结果或聊天室已不存在')
  if (response.status === 409) throw new AiExtractionApiError(409, '候选项已在其他窗口中处理')
  if (response.status === 429) throw new AiExtractionApiError(429, 'AI 并发或当日用量已达上限，请稍后再试')
  if (response.status === 503) throw new AiExtractionApiError(503, '所选 AI 模型不可用或未被部署允许')
  if (!response.ok) throw new AiExtractionApiError(response.status, `AI 提取失败：${response.status}`)
  return response
}

export async function createAiExtraction(
  roomId: string,
  token: string,
  password: string,
  fromAt: string,
  toAt: string,
  modelOptionId: string | null,
): Promise<AiExtractionRun> {
  const response = await request(`/api/rooms/${encodeURIComponent(roomId)}/ai/extractions`, {
    method: 'POST',
    headers: headers(token, password, true),
    body: JSON.stringify({
      from_at: fromAt,
      to_at: toAt,
      model_option_id: modelOptionId,
      client_request_id: crypto.randomUUID(),
    }),
  })
  return checked(response).then((value) => value.json() as Promise<AiExtractionRun>)
}

export async function getAiExtraction(runId: string, token: string, password: string): Promise<AiExtractionRun> {
  const response = await request(`/api/ai/extractions/${encodeURIComponent(runId)}`, {
    headers: headers(token, password),
  })
  return checked(response).then((value) => value.json() as Promise<AiExtractionRun>)
}

export async function updateAiExtractionCandidate(
  candidate: AiExtractionCandidate,
  action: 'confirm' | 'dismiss',
  token: string,
  password: string,
): Promise<AiExtractionCandidate> {
  const response = await request(`/api/ai/extraction-candidates/${encodeURIComponent(candidate.id)}`, {
    method: 'PATCH',
    headers: headers(token, password, true),
    body: JSON.stringify({ action, version: candidate.version }),
  })
  return checked(response).then((value) => value.json() as Promise<AiExtractionCandidate>)
}
