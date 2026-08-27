import { citedAiSources, formatLocalDateTime } from './aiUi'
import type { AiThreadMessage } from './types'

const MAX_ANSWER_CHARS = 4_800
const MAX_SOURCES = 10

function truncate(value: string, limit: number): string {
  const characters = Array.from(value)
  return characters.length <= limit ? value : `${characters.slice(0, limit - 1).join('')}…`
}

export function aiFavoriteTitle(roomTitle: string): string {
  return `${roomTitle.trim() || '聊天会话'} · AI 回答`
}

export function aiFavoriteContent(message: AiThreadMessage): string {
  const sources = citedAiSources(message.content, message.sources)
  const sourceLines = sources.slice(0, MAX_SOURCES).map((source) => {
    const room = encodeURIComponent(source.room_id)
    const messageId = encodeURIComponent(source.message_id)
    const url = `/rooms/${room}?message=${messageId}#message-${messageId}`
    return `- [${source.label} · ${source.sender}](${url}) · ${formatLocalDateTime(source.sent_at)}\n  ${truncate(source.excerpt, 160)}`
  })
  if (sources.length > MAX_SOURCES)
    sourceLines.push(`- 另有 ${sources.length - MAX_SOURCES} 条引用，请在 AI 对话中查看`)
  return [
    truncate(message.content.trim(), MAX_ANSWER_CHARS),
    sourceLines.length ? `来源\n${sourceLines.join('\n')}` : '',
  ]
    .filter(Boolean)
    .join('\n\n')
}
