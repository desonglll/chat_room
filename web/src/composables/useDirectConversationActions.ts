import { toValue, type MaybeRefOrGetter } from 'vue'
import { startDirectChat } from '../socialApi'
import type { ConversationSummary } from '../types'

interface DirectConversationOptions {
  token: MaybeRefOrGetter<string>
  selectedConversation: MaybeRefOrGetter<ConversationSummary | null>
  selectConversation: (conversation: ConversationSummary) => void
  clearSelection: () => void
  refreshConversations: () => Promise<void>
  setRemark: (userId: string, remark: string) => Promise<void>
}

export function useDirectConversationActions(options: DirectConversationOptions) {
  async function openDirectConversation(userId: string): Promise<void> {
    options.selectConversation(await startDirectChat(userId, toValue(options.token)))
  }

  async function changeDirectAccess(userId: string, action: (id: string) => Promise<void>): Promise<void> {
    await action(userId)
    if (toValue(options.selectedConversation)?.peer?.id === userId) options.clearSelection()
    await options.refreshConversations()
  }

  async function setSelectedFriendRemark(userId: string, remark: string): Promise<void> {
    await options.setRemark(userId, remark)
    await options.refreshConversations()
  }

  function changeSelectedDirectAccess(action: (id: string) => Promise<void>): void {
    const userId = toValue(options.selectedConversation)?.peer?.id
    if (userId) void changeDirectAccess(userId, action)
  }

  return { changeDirectAccess, changeSelectedDirectAccess, openDirectConversation, setSelectedFriendRemark }
}
