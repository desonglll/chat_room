import type { BroadcastMessage, DisplayMessage, UploadMessage } from './types'

export function appendUploadMessage(messages: DisplayMessage[], upload: UploadMessage): DisplayMessage[] {
  const exists = messages.some((message) => message.type === 'upload' && message.key === upload.key)
  return exists ? messages : [...messages, upload]
}

export function updateUploadMessage(
  messages: DisplayMessage[],
  key: string,
  patch: Partial<UploadMessage>,
): DisplayMessage[] {
  return messages.map((message) =>
    message.type === 'upload' && message.key === key ? { ...message, ...patch, type: 'upload', key } : message,
  )
}

export function completeUploadMessage(
  messages: DisplayMessage[],
  key: string,
  message: BroadcastMessage,
): DisplayMessage[] | null {
  if (!messages.some((item) => item.type === 'upload' && item.key === key)) return null
  return messages.flatMap((item) => {
    if (item.type === 'upload' && item.key === key) return [{ ...message, motion: 'outgoing' as const }]
    if (item.type === 'broadcast' && item.message_id === message.message_id) return []
    return [item]
  })
}

export function removeUploadMessage(messages: DisplayMessage[], key: string): DisplayMessage[] {
  return messages.filter((message) => message.type !== 'upload' || message.key !== key)
}
