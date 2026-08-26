import type { AiCitationSource, AiRunStage, AiThreadMessage } from './types'

export type AiUiMessage = AiThreadMessage

export function aiContextUsage(
  totalMessageCount: number | null,
  retrievedMessageCount: number | null,
): { recent: number; retrieved: number } {
  const total = Math.max(0, totalMessageCount || 0)
  const retrieved = Math.min(total, Math.max(0, retrievedMessageCount || 0))
  return { recent: total - retrieved, retrieved }
}

export function aiSourceRoute(source: AiCitationSource) {
  return {
    name: 'room' as const,
    params: { id: source.room_id },
    query: { message: source.message_id },
    hash: `#message-${source.message_id}`,
  }
}

export function citedAiSources(content: string, sources: AiCitationSource[]): AiCitationSource[] {
  const labels = new Set(Array.from(content.matchAll(/\[([a-z]\d+)\]/gi), (match) => match[1].toLocaleUpperCase()))
  return sources.filter((source) => labels.has(source.label.toLocaleUpperCase()))
}

export function ragAiSources(sources: AiCitationSource[]): AiCitationSource[] {
  return sources.filter((source) => source.score_kind !== 'attachment')
}

export function referencedAiAttachments(content: string, sources: AiCitationSource[]): AiCitationSource[] {
  const byLabel = new Map(sources.map((source) => [source.label.toLocaleUpperCase(), source]))
  const seen = new Set<string>()
  const referenced: AiCitationSource[] = []
  for (const match of content.matchAll(/\[([a-z]\d+)\]/gi)) {
    const source = byLabel.get(match[1].toLocaleUpperCase())
    const attachment = source?.attachment
    if (!attachment || seen.has(attachment.id)) continue
    seen.add(attachment.id)
    referenced.push(source)
  }
  for (const source of sources) {
    const attachment = source.attachment
    if (!attachment || seen.has(attachment.id) || !content.includes(attachment.file_name)) continue
    seen.add(attachment.id)
    referenced.push(source)
  }
  return referenced
}

export function inlineAiAttachments(content: string, sources: AiCitationSource[]): AiCitationSource[] {
  const citedLabels = new Set(citedAiSources(content, sources).map((source) => source.label.toLocaleUpperCase()))
  return referencedAiAttachments(content, sources).filter((source) => citedLabels.has(source.label.toLocaleUpperCase()))
}

export function trailingAiAttachments(content: string, sources: AiCitationSource[]): AiCitationSource[] {
  const inlineAttachmentIds = new Set(
    inlineAiAttachments(content, sources)
      .map((source) => source.attachment?.id)
      .filter(Boolean),
  )
  return referencedAiAttachments(content, sources).filter(
    (source) => !source.attachment || !inlineAttachmentIds.has(source.attachment.id),
  )
}

export function localTimeZone(): string {
  return Intl.DateTimeFormat().resolvedOptions().timeZone
}

export function formatLocalDateTime(value: string, timeZone = localTimeZone()): string {
  const trimmed = value.trim()
  const normalized = /(?:z|[+-]\d{2}:?\d{2})$/i.test(trimmed) ? trimmed : `${trimmed.replace(' ', 'T')}Z`
  const date = new Date(normalized)
  if (Number.isNaN(date.getTime())) return ''
  return new Intl.DateTimeFormat('zh-CN', {
    dateStyle: 'medium',
    timeStyle: 'short',
    timeZone,
  }).format(date)
}

const stageLabels: Record<AiRunStage, string> = {
  queued: '请求已排队',
  preparing_context: '正在准备对话上下文',
  retrieving_context: '正在向量化并检索历史消息',
  connecting_model: '正在连接模型',
  waiting_for_model: '等待模型首个响应',
  reasoning: '模型正在思考',
  responding: '正在生成回答',
  completed: '回答完成',
  failed: '请求失败',
}

export function aiRunStageStatus(
  message: Pick<AiThreadMessage, 'stage' | 'stage_started_at' | 'created_at'>,
  now = Date.now(),
): { label: string; stageSeconds: number; totalSeconds: number } {
  const startedAt = Date.parse(message.created_at)
  const stageStartedAt = Date.parse(message.stage_started_at || message.created_at)
  const elapsed = (value: number) => Math.max(0, Math.floor((now - value) / 1000))
  return {
    label: stageLabels[message.stage],
    stageSeconds: elapsed(stageStartedAt),
    totalSeconds: elapsed(startedAt),
  }
}
