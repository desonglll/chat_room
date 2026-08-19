import { describe, expect, test } from 'bun:test'
import { createOptimisticMessage, reconcileOptimisticMessage, updateDeliveryState } from './chatOptimistic'
import type { BroadcastMessage } from './types'

describe('optimistic messages', () => {
  test('creates a local outgoing message and reconciles it by client id', () => {
    const pending = createOptimisticMessage({
      clientMessageId: 'client-1',
      content: 'hello',
      replyTo: '',
      currentUserId: 'user-1',
      participants: [{ user_id: 'user-1', username: 'mike', avatar_emoji: 'M' }],
      messages: [],
      timestamp: '2026-08-19T00:00:00Z',
    })
    expect(pending.delivery_state).toBe('sending')
    expect(pending.sender).toBe('mike')

    const confirmed: BroadcastMessage = {
      ...pending,
      message_id: 'server-1',
      delivery_state: undefined,
    }
    const result = reconcileOptimisticMessage([pending], confirmed)
    expect(result.matched).toBe(true)
    expect((result.messages[0] as BroadcastMessage).message_id).toBe('server-1')
    expect((result.messages[0] as BroadcastMessage).delivery_state).toBe('sent')
  })

  test('marks an unacknowledged message as failed without changing its client id', () => {
    const pending = createOptimisticMessage({
      clientMessageId: 'client-2',
      content: 'retry me',
      replyTo: '',
      currentUserId: 'user-1',
      participants: [],
      messages: [],
    })
    const failed = updateDeliveryState([pending], 'client-2', 'failed')[0] as BroadcastMessage
    expect(failed.delivery_state).toBe('failed')
    expect(failed.client_message_id).toBe('client-2')
  })
})
