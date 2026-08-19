import { describe, expect, it } from 'vitest'
import {
  applyAccountMessage,
  applyAccountStates,
  conversationPreview,
  conversationToRoom,
  sortConversations,
} from './conversationState'
import type { AccountMessageEvent, ConversationSummary } from './types'

function conversation(overrides: Partial<ConversationSummary> = {}): ConversationSummary {
  return {
    room_id: 'room-1',
    kind: 'group',
    title: '研发群',
    avatar_emoji: '',
    description: '',
    group: null,
    peer: null,
    unread_count: 0,
    pending_join_requests: 0,
    last_message: null,
    last_activity_at: '2026-08-19T08:00:00Z',
    created_at: '2026-08-18T08:00:00Z',
    ...overrides,
  }
}

describe('conversation state', () => {
  it('sorts the most recently active conversation first without mutating input', () => {
    const older = conversation({ room_id: 'older' })
    const newer = conversation({ room_id: 'newer', last_activity_at: '2026-08-19T09:00:00Z' })
    const source = [older, newer]

    expect(sortConversations(source).map((item) => item.room_id)).toEqual(['newer', 'older'])
    expect(source).toEqual([older, newer])
  })

  it('moves a conversation to the top and updates its viewer-specific title', () => {
    const event: AccountMessageEvent = {
      type: 'new_message',
      message_id: 'message-2',
      room_id: 'room-1',
      room_name: 'internal-room-name',
      conversation_kind: 'direct',
      conversation_title: '小林',
      sender_id: 'peer-1',
      sender: 'lin',
      content: '晚上见',
      attachment_file_name: null,
      timestamp: '2026-08-19T10:00:00Z',
      is_mention: false,
    }

    const result = applyAccountMessage([conversation(), conversation({ room_id: 'room-2' })], event, 'room-2')

    expect(result[0]).toMatchObject({ room_id: 'room-1', kind: 'direct', title: '小林' })
    expect(result[0].last_message).toMatchObject({ message_id: 'message-2', content: '晚上见' })
    expect(result[0].unread_count).toBe(1)
  })

  it('projects a direct conversation into an active two-person room view', () => {
    const result = conversationToRoom(
      conversation({
        room_id: 'direct-1',
        kind: 'direct',
        title: '小林',
        avatar_emoji: 'L',
        peer: {
          id: 'peer-1',
          username: 'lin',
          avatar_emoji: 'L',
          display_name: '小林',
        },
      }),
    )

    expect(result).toMatchObject({
      id: 'direct-1',
      name: '小林',
      has_password: false,
      membership_status: 'active',
      membership_role: 'member',
    })
  })

  it('surfaces pending join requests ahead of the latest room message', () => {
    const pending = conversation({
      pending_join_requests: 2,
      last_message: {
        message_id: 'message-1',
        sender_id: 'member-1',
        sender: '成员',
        content: '普通消息',
        attachment_file_name: null,
        recalled: false,
        created_at: '2026-08-19T08:30:00Z',
      },
    })

    expect(conversationPreview(pending)).toBe('2 条入群申请')
  })

  it('moves a room when an account snapshot reports a new join request', () => {
    const result = applyAccountStates(
      [conversation(), conversation({ room_id: 'room-2', last_activity_at: '2026-08-19T09:00:00Z' })],
      new Map([
        [
          'room-1',
          {
            unread_count: 0,
            pending_join_requests: 1,
            pending_join_requested_at: '2026-08-19T10:00:00Z',
          },
        ],
      ]),
    )

    expect(result[0]).toMatchObject({ room_id: 'room-1', pending_join_requests: 1 })
    expect(conversationPreview(result[0] as ConversationSummary)).toBe('1 条入群申请')
  })
})
