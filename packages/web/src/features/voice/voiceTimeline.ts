/**
 * Voice messages inside the open chats' timelines (`messageStore`): which chat holds a voice
 * message, and which voice message follows it — "auto-play next" walks forward from the one
 * that just ended, like Telegram.
 */
import type { BroadcastMessage, DisplayMessage, MessageStore } from '@tg/core'
import type { VoiceTrack } from './voicePlayer'

export function isVoiceMessage(message: DisplayMessage): message is BroadcastMessage & {
  voice: NonNullable<BroadcastMessage['voice']>
  attachment: NonNullable<BroadcastMessage['attachment']>
} {
  return message.type === 'broadcast' && message.voice != null && message.attachment !== null && !message.recalled_at
}

export function voiceTrack(message: BroadcastMessage): VoiceTrack | null {
  if (!isVoiceMessage(message)) return null
  return { messageId: message.message_id, url: message.attachment.download_url, durationMs: message.voice.duration_ms }
}

export interface LocatedVoice {
  chatId: string
  message: BroadcastMessage
}

export function locateVoice(messages: MessageStore, messageId: string): LocatedVoice | null {
  for (const [chatId, timeline] of Object.entries(messages.getState().timelines)) {
    const message = timeline.messages.find(
      (candidate): candidate is BroadcastMessage =>
        candidate.type === 'broadcast' && candidate.message_id === messageId,
    )
    if (message) return { chatId, message }
  }
  return null
}

export function nextVoiceTrack(messages: MessageStore, messageId: string): VoiceTrack | null {
  for (const timeline of Object.values(messages.getState().timelines)) {
    const index = timeline.messages.findIndex(
      (candidate) => candidate.type === 'broadcast' && candidate.message_id === messageId,
    )
    if (index < 0) continue
    for (const candidate of timeline.messages.slice(index + 1)) {
      if (isVoiceMessage(candidate)) return voiceTrack(candidate)
    }
    return null
  }
  return null
}
