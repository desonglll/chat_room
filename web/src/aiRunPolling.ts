import type { AiThreadMessage } from './types'

export interface AiPollingOptions {
  intervalMs?: number
  isCurrent?: () => boolean
}

export function hasActiveAiMessage(messages: AiThreadMessage[]): boolean {
  return messages.some((message) => message.status === 'pending' || message.status === 'streaming')
}

export async function pollAiThreadMessages(
  load: () => Promise<AiThreadMessage[]>,
  onUpdate: (messages: AiThreadMessage[]) => void,
  options: AiPollingOptions = {},
): Promise<AiThreadMessage[]> {
  const intervalMs = options.intervalMs ?? 100
  while (options.isCurrent?.() !== false) {
    const messages = await load()
    onUpdate(messages)
    if (!hasActiveAiMessage(messages)) return messages
    await new Promise((resolve) => setTimeout(resolve, intervalMs))
  }
  return []
}
