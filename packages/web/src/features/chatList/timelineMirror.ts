/**
 * Open chat timelines → sidebar previews. The newest settled `broadcast` of each timeline
 * becomes that chat's last message (own sends, recalls and edits included — the account
 * socket covers none of those). Framework-free; returns the unsubscribe function.
 */
import type { ChatListStore, ConversationLastMessage, DisplayMessage, MessageStore } from '@tg/core'

export function latestSettledMessage(messages: readonly DisplayMessage[]): ConversationLastMessage | null {
  for (let index = messages.length - 1; index >= 0; index -= 1) {
    const message = messages[index]
    if (message?.type !== 'broadcast') continue
    if (message.delivery_state === 'sending' || message.delivery_state === 'failed') continue
    return {
      message_id: message.message_id,
      sender_id: message.sender_id,
      sender: message.sender,
      content: message.content.slice(0, 120),
      attachment_file_name: message.attachment?.file_name ?? null,
      recalled: message.recalled_at !== null,
      created_at: message.timestamp,
    }
  }
  return null
}

export function mirrorTimelines(messages: MessageStore, chatList: ChatListStore): () => void {
  return messages.subscribe((state, previous) => {
    for (const [chatId, timeline] of Object.entries(state.timelines)) {
      if (previous.timelines[chatId] === timeline) continue
      const latest = latestSettledMessage(timeline.messages)
      if (latest) chatList.getState().applyLatestMessage(chatId, latest)
    }
  })
}
