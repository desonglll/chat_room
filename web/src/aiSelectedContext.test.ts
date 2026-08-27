import { describe, expect, test } from 'bun:test'
import { selectedAiMessages } from './aiSelectedContext'
import type { DisplayMessage } from './types'

const messages: DisplayMessage[] = [
  {
    type: 'broadcast',
    message_id: 'message-1',
    sender_id: 'user-1',
    sender: 'Ada',
    sender_avatar: '',
    content: 'first',
    attachment: null,
    reply_to: null,
    recalled_at: null,
    edited_at: null,
    timestamp: '2026-08-27T10:00:00Z',
    forwarded_from: null,
    reactions: [],
  },
  { type: 'system', key: 'joined', content: 'joined' },
  {
    type: 'broadcast',
    message_id: 'message-2',
    sender_id: 'user-2',
    sender: 'Lin',
    sender_avatar: '',
    content: '',
    reply_to: null,
    recalled_at: null,
    edited_at: null,
    timestamp: '2026-08-27T10:02:00Z',
    attachment: {
      id: 'file-1',
      file_name: 'plan.pdf',
      mime_type: 'application/pdf',
      size_bytes: 10,
      download_url: '/file',
      is_sensitive: false,
    },
    forwarded_from: null,
    reactions: [],
  },
]

describe('selected AI message context', () => {
  test('keeps the explicit selection order and omits non-message entries', () => {
    expect(selectedAiMessages(messages, ['message-2', 'missing', 'message-1'])).toEqual([
      {
        messageId: 'message-2',
        sender: 'Lin',
        preview: '[附件] plan.pdf',
        sentAt: '2026-08-27T10:02:00Z',
      },
      { messageId: 'message-1', sender: 'Ada', preview: 'first', sentAt: '2026-08-27T10:00:00Z' },
    ])
  })
})
