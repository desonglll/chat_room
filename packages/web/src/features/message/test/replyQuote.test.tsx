import { describe, expect, test } from 'bun:test'
import { renderToStaticMarkup } from 'react-dom/server'
import type { BroadcastMessage, ReplyPreview } from '@tg/core'
import { ReplyQuote } from '../BubbleParts'
import { buildMessageMenu } from '../messageMenu'

const base: ReplyPreview = {
  message_id: 'm1',
  sender: 'Alice',
  content: 'top secret plan: launch at dawn',
  attachment_file_name: null,
  recalled: false,
}

describe('TG-409 reply quotes', () => {
  test('a quote shows the quoted slice, not the whole original, and flags a modified original', () => {
    const html = renderToStaticMarkup(
      <ReplyQuote reply={{ ...base, quote: { text: 'launch at dawn', offset: 17 }, quote_modified: true }} />,
    )
    expect(html).toContain('launch at dawn')
    expect(html).not.toContain('top secret')
    expect(html).toContain('已修改')
    expect(html).toContain('data-quote')
  })

  test('a cross-chat reply names its source chat', () => {
    const html = renderToStaticMarkup(
      <ReplyQuote reply={{ ...base, content: 'top secret plan', chat_id: 'c9', chat_title: '项目组' }} />,
    )
    expect(html).toContain('项目组')
    expect(html).toContain('Alice')
  })

  test('the menu offers 引用 and 在其他聊天中回复 only when their actions are bound', () => {
    const message = {
      type: 'broadcast',
      message_id: 'm1',
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
    } as BroadcastMessage
    const ids = (actions: Parameters<typeof buildMessageMenu>[1]) =>
      buildMessageMenu(message, actions, { delivered: true }).map((item) => item.id)
    expect(ids({})).not.toContain('quote')
    expect(ids({ onQuote: () => {}, onReplyElsewhere: () => {} })).toEqual(
      expect.arrayContaining(['quote', 'reply-elsewhere']),
    )
  })
})
