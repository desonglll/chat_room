import type { Ref } from 'vue'
import {
  updateConversationPreferences,
  type ConversationPreferences,
  type ConversationPreferencesPatch,
} from '../conversationPreferencesApi'
import { sortConversations } from '../conversationState'
import type { ConversationSummary } from '../types'

type PreferenceWriter = (
  roomId: string,
  patch: ConversationPreferencesPatch,
  token: string,
) => Promise<ConversationPreferences>

export function useConversationPreferences(
  conversations: Ref<ConversationSummary[]>,
  token: Ref<string>,
  writePreferences: PreferenceWriter = updateConversationPreferences,
) {
  async function update(roomId: string, patch: ConversationPreferencesPatch): Promise<ConversationPreferences> {
    const current = conversations.value.find((item) => item.room_id === roomId)
    if (!current) throw new Error('会话已失效，请刷新后重试')
    const previous = current.preferences
    const optimistic = { ...previous, ...patch, updated_at: new Date().toISOString() }
    replace(roomId, optimistic)
    try {
      const saved = await writePreferences(roomId, patch, token.value)
      replace(roomId, saved)
      return saved
    } catch (error) {
      replace(roomId, previous)
      throw error
    }
  }

  function replace(roomId: string, preferences: ConversationPreferences): void {
    conversations.value = sortConversations(
      conversations.value.map((item) => (item.room_id === roomId ? { ...item, preferences } : item)),
    )
  }

  return { update }
}
