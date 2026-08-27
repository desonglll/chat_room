import { AdminApiError } from './adminApi'

export interface AiGovernedModel {
  id: string
  label: string
  provider: string
  model: string
  ready: boolean
  allowed: boolean
  input_price_micros_per_million: number
  output_price_micros_per_million: number
}

export interface AiGovernanceSettings {
  max_concurrent_runs: number
  daily_user_token_limit: number | null
  daily_room_token_limit: number | null
  allowlist_enabled: boolean
  models: AiGovernedModel[]
  updated_at: string
}

export interface AiUsageAggregate {
  key: string
  label: string
  runs: number
  completed_runs: number
  failed_runs: number
  input_tokens: number
  output_tokens: number
  total_tokens: number
  duration_ms: number
  estimated_cost_micros: number
}

export interface AiUsageReport {
  group_by: 'room' | 'model'
  from: string
  to: string
  token_source: 'estimated'
  items: AiUsageAggregate[]
}

async function governanceRequest(path: string, token: string, method = 'GET', body?: unknown): Promise<Response> {
  const response = await fetch(path, {
    method,
    cache: 'no-store',
    headers: {
      Accept: 'application/json',
      Authorization: `Bearer ${token}`,
      ...(body ? { 'Content-Type': 'application/json' } : {}),
    },
    body: body ? JSON.stringify(body) : undefined,
  })
  if (!response.ok) throw new AdminApiError(response.status, response.status === 400 ? 'AI 治理设置无效' : undefined)
  return response
}

export async function getAiGovernance(token: string): Promise<AiGovernanceSettings> {
  return (await governanceRequest('/api/admin/ai-governance', token)).json() as Promise<AiGovernanceSettings>
}

export async function saveAiGovernance(token: string, settings: AiGovernanceSettings): Promise<AiGovernanceSettings> {
  const payload = {
    max_concurrent_runs: settings.max_concurrent_runs,
    daily_user_token_limit: settings.daily_user_token_limit,
    daily_room_token_limit: settings.daily_room_token_limit,
    allowlist_enabled: settings.allowlist_enabled,
    models: settings.models.map((model) => ({
      id: model.id,
      allowed: model.allowed,
      input_price_micros_per_million: model.input_price_micros_per_million,
      output_price_micros_per_million: model.output_price_micros_per_million,
    })),
  }
  return (
    await governanceRequest('/api/admin/ai-governance', token, 'PATCH', payload)
  ).json() as Promise<AiGovernanceSettings>
}

export async function getAiUsage(token: string, groupBy: 'room' | 'model'): Promise<AiUsageReport> {
  return (
    await governanceRequest(`/api/admin/ai-usage?group_by=${encodeURIComponent(groupBy)}`, token)
  ).json() as Promise<AiUsageReport>
}
