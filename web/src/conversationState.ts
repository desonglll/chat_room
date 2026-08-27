import type { AccountMessageEvent, ConversationSummary, Room } from './types'

export interface ConversationAccountState {
  unread_count: number
  membership_status: 'pending' | 'invited' | 'active'
  pending_join_requests: number
  pending_join_requested_at: string | null
}

export function sortConversations(conversations: readonly ConversationSummary[]): ConversationSummary[] {
  return [...conversations].sort((left, right) => {
    if (left.preferences.is_archived !== right.preferences.is_archived) {
      return Number(left.preferences.is_archived) - Number(right.preferences.is_archived)
    }
    if (!left.preferences.is_archived && left.preferences.is_pinned !== right.preferences.is_pinned) {
      return Number(right.preferences.is_pinned) - Number(left.preferences.is_pinned)
    }
    const activity = right.last_activity_at.localeCompare(left.last_activity_at)
    return activity || left.room_id.localeCompare(right.room_id)
  })
}

export function isConversationMuted(conversation: ConversationSummary, now = Date.now()): boolean {
  if (conversation.preferences.notification_level === 'none') return true
  const mutedUntil = conversation.preferences.muted_until
  return Boolean(mutedUntil && Date.parse(mutedUntil) > now)
}

export function removeConversation(
  conversations: readonly ConversationSummary[],
  roomId: string,
): ConversationSummary[] {
  return conversations.filter((conversation) => conversation.room_id !== roomId)
}

export function applyAccountMessage(
  conversations: readonly ConversationSummary[],
  event: AccountMessageEvent,
  activeRoomId = '',
): ConversationSummary[] {
  const existing = conversations.find((conversation) => conversation.room_id === event.room_id)
  if (!existing) return [...conversations]
  const updated: ConversationSummary = {
    ...existing,
    kind: event.conversation_kind,
    title: event.conversation_title,
    unread_count: activeRoomId === event.room_id ? 0 : existing.unread_count + 1,
    last_activity_at: event.timestamp,
    last_message: {
      message_id: event.message_id,
      sender_id: event.sender_id,
      sender: event.sender,
      content: event.content,
      attachment_file_name: event.attachment_file_name,
      recalled: false,
      created_at: event.timestamp,
    },
  }
  return sortConversations(
    conversations.map((conversation) => (conversation.room_id === event.room_id ? updated : conversation)),
  )
}

export function applyAccountStates(
  conversations: readonly ConversationSummary[],
  states: ReadonlyMap<string, ConversationAccountState>,
): ConversationSummary[] {
  return sortConversations(
    conversations
      .filter((conversation) => states.get(conversation.room_id)?.membership_status === 'active')
      .map((conversation) => {
        const state = states.get(conversation.room_id)!
        const requestActivity = state.pending_join_requested_at || ''
        return {
          ...conversation,
          unread_count: state.unread_count,
          pending_join_requests: state.pending_join_requests,
          last_activity_at:
            requestActivity > conversation.last_activity_at ? requestActivity : conversation.last_activity_at,
        }
      }),
  )
}

export function conversationToRoom(conversation: ConversationSummary): Room {
  const name = conversationDisplayTitle(conversation)
  if (conversation.kind === 'group' && conversation.group) {
    return { ...conversation.group, name, unread_count: conversation.unread_count }
  }
  return {
    id: conversation.room_id,
    name,
    has_password: false,
    creator_user_id: null,
    join_policy: 'approval',
    avatar_emoji: conversation.avatar_emoji,
    description: conversation.description,
    membership_status: 'active',
    membership_role: 'member',
    unread_count: conversation.unread_count,
    created_at: conversation.created_at,
  }
}

export function conversationDisplayTitle(conversation: ConversationSummary): string {
  return conversation.alias || conversation.title
}

export function conversationPreview(conversation: ConversationSummary, revealContent = true): string {
  if (conversation.pending_join_requests > 0) return `${conversation.pending_join_requests} 条入群申请`
  if (!revealContent) return ''
  const message = conversation.last_message
  if (!message) return conversation.kind === 'direct' ? '开始聊天' : conversation.description || '暂无消息'
  if (message.recalled) return '消息已撤回'
  if (message.content.trim()) return message.content.trim()
  if (message.attachment_file_name) return `文件：${message.attachment_file_name}`
  return '新消息'
}

export function conversationAttentionCount(conversation: ConversationSummary): number {
  return conversation.unread_count + conversation.pending_join_requests
}

export function shouldRevealConversationPreview(activeSection: string, selectedId?: string): boolean {
  return activeSection === 'chat' && Boolean(selectedId)
}
