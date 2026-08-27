import type { BroadcastMessage, DisplayMessage } from './types'

const PREFIX = 'chat-room.conversation-draft.v1'

export interface ConversationDraft {
  content: string
  reply_to_message_id: string | null
  updated_at: string
}

export interface ConversationDraftStorage {
  read(userId: string, roomId: string): ConversationDraft | null
  write(userId: string, roomId: string, content: string, replyToMessageId: string | null): void
}

function keyOf(userId: string, roomId: string): string {
  return `${PREFIX}.${encodeURIComponent(userId)}.${encodeURIComponent(roomId)}`
}

function parseDraft(value: string): ConversationDraft | null {
  try {
    const draft = JSON.parse(value) as Partial<ConversationDraft>
    if (typeof draft.content !== 'string' || typeof draft.updated_at !== 'string') return null
    if (draft.reply_to_message_id !== null && typeof draft.reply_to_message_id !== 'string') return null
    return {
      content: draft.content.slice(0, 4096),
      reply_to_message_id: draft.reply_to_message_id?.slice(0, 128) || null,
      updated_at: draft.updated_at,
    }
  } catch {
    return null
  }
}

export function createConversationDraftStorage(
  storage: Storage | null,
  now: () => string = () => new Date().toISOString(),
): ConversationDraftStorage {
  return {
    read(userId, roomId) {
      if (!storage || !userId || !roomId) return null
      try {
        const value = storage.getItem(keyOf(userId, roomId))
        return value ? parseDraft(value) : null
      } catch {
        return null
      }
    },
    write(userId, roomId, content, replyToMessageId) {
      if (!storage || !userId || !roomId) return
      try {
        const key = keyOf(userId, roomId)
        if (!content && !replyToMessageId) {
          storage.removeItem(key)
          return
        }
        const draft: ConversationDraft = {
          content: content.slice(0, 4096),
          reply_to_message_id: replyToMessageId?.slice(0, 128) || null,
          updated_at: now(),
        }
        storage.setItem(key, JSON.stringify(draft))
      } catch {
        // Draft persistence is optional; storage failures must not block typing.
      }
    },
  }
}

export function resolveDraftReply(messages: readonly DisplayMessage[], messageId: string): BroadcastMessage | null {
  const message = messages.find(
    (candidate): candidate is BroadcastMessage => candidate.type === 'broadcast' && candidate.message_id === messageId,
  )
  return message && !message.recalled_at ? message : null
}
