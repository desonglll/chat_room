import { computed, ref, watch, type Ref } from 'vue'
import { applyAccountMessage, applyAccountStates, sortConversations } from '../conversationState'
import { listConversations } from '../socialApi'
import type { AccountMessageEvent, ConversationSummary } from '../types'

interface UnreadState {
  unread_count: number
  pending_join_requests: number
  pending_join_requested_at: string | null
}

export function useConversations(token: Ref<string>, activeRoomId: Ref<string | undefined>) {
  const conversations = ref<ConversationSummary[]>([])
  const loading = ref(false)
  const error = ref('')
  let requestVersion = 0

  async function refresh(): Promise<void> {
    const activeToken = token.value
    const version = ++requestVersion
    if (!activeToken) {
      conversations.value = []
      error.value = ''
      return
    }
    loading.value = true
    try {
      const next = await listConversations(activeToken)
      if (version !== requestVersion || activeToken !== token.value) return
      conversations.value = sortConversations(next)
      error.value = ''
    } catch (caught) {
      if (version !== requestVersion) return
      error.value = caught instanceof Error ? caught.message : '无法读取会话'
    } finally {
      if (version === requestVersion) loading.value = false
    }
  }

  function applyUnread(states: Map<string, UnreadState>): void {
    conversations.value = applyAccountStates(conversations.value, states)
  }

  function handleMessage(event: AccountMessageEvent): void {
    if (!conversations.value.some((conversation) => conversation.room_id === event.room_id)) {
      void refresh()
      return
    }
    conversations.value = applyAccountMessage(conversations.value, event, activeRoomId.value)
  }

  function upsert(conversation: ConversationSummary): void {
    conversations.value = sortConversations([
      conversation,
      ...conversations.value.filter((item) => item.room_id !== conversation.room_id),
    ])
  }

  watch(token, () => void refresh(), { immediate: true })

  return {
    conversations,
    error,
    loading,
    totalUnread: computed(() => conversations.value.reduce((sum, item) => sum + item.unread_count, 0)),
    applyUnread,
    handleMessage,
    refresh,
    upsert,
  }
}
