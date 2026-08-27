import { nextTick, watch, type Ref } from 'vue'
import { createConversationDraftStorage, resolveDraftReply } from '../conversationDraftStorage'
import type { ConversationDraftStorage } from '../conversationDraftStorage'
import type { BroadcastMessage, DisplayMessage } from '../types'

export interface ConversationDraftContext {
  userId: string
  ready: boolean
}

interface ConversationDraftProps {
  draftContext: ConversationDraftContext
  roomId: string
  messages: DisplayMessage[]
  replyingTo: BroadcastMessage | null
  editingTo: BroadcastMessage | null
}

interface ConversationDraftOptions {
  draft: Ref<string>
  updateReply: (message: BroadcastMessage | null) => void
  editingLoaded: () => void
}

function browserDraftStorage(): ConversationDraftStorage {
  try {
    return createConversationDraftStorage(window.localStorage)
  } catch {
    return createConversationDraftStorage(null)
  }
}

export function useConversationDraft(
  props: ConversationDraftProps,
  options: ConversationDraftOptions,
  storage = browserDraftStorage(),
): void {
  let activeScope = ''

  watch([options.draft, () => props.replyingTo?.message_id], ([content, replyToMessageId]) => {
    if (!activeScope || props.editingTo) return
    storage.write(props.draftContext.userId, props.roomId, content, replyToMessageId || null)
  })

  function restore(force = false): void {
    const { ready, userId } = props.draftContext
    if (!ready || !userId || !props.roomId || props.editingTo) return
    const scope = `${userId}:${props.roomId}`
    if (!force && activeScope === scope) return
    activeScope = scope
    const stored = storage.read(userId, props.roomId)
    options.draft.value = stored?.content || ''
    const reply = stored?.reply_to_message_id ? resolveDraftReply(props.messages, stored.reply_to_message_id) : null
    options.updateReply(reply)
    if (stored?.reply_to_message_id && !reply) storage.write(userId, props.roomId, stored.content, null)
  }

  watch([() => props.draftContext.userId, () => props.roomId, () => props.draftContext.ready], () => restore(), {
    immediate: true,
  })

  watch(
    () => props.editingTo?.message_id,
    (messageId, previousMessageId) => {
      if (!messageId || !props.editingTo) {
        if (previousMessageId) restore(true)
        return
      }
      options.draft.value = props.editingTo.content
      void nextTick(options.editingLoaded)
    },
  )
}
