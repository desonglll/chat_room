import { describe, expect, test } from 'bun:test'
import type { BroadcastMessage } from '@tg/core'
import { buildMessageMenu } from '../../message/messageMenu'
import '../register'

function message(id: string): BroadcastMessage {
  return {
    type: 'broadcast',
    message_id: id,
    sender_id: 'u1',
    sender: 'Alice',
    sender_avatar: '',
    content: 'hello',
    attachment: null,
    reply_to: null,
    recalled_at: null,
    edited_at: null,
    timestamp: '2026-10-01T00:00:00Z',
    favorite_id: null,
    forwarded_from: null,
    reactions: [],
  }
}

describe('Saved Messages entry point', () => {
  test('«保存到收藏夹» is offered on a delivered message, never on a pending one', () => {
    const delivered = buildMessageMenu(message('m1'), {}, { delivered: true }).map((item) => item.id)
    expect(delivered).toContain('save-to-favorites')
    const pending = buildMessageMenu(message('pending:x'), {}, { delivered: false }).map((item) => item.id)
    expect(pending).not.toContain('save-to-favorites')
  })
})
