import { describe, expect, test } from 'bun:test'
import { renderToStaticMarkup } from 'react-dom/server'
import { MemoryRouter } from 'react-router-dom'
import type { BroadcastMessage } from '@tg/core'
import { buildMessageMenu } from '../../message/messageMenu'
import { ContactContent } from '../ContactContent'
import { translationStore } from '../translation'
import '../register'

function message(extra: Partial<BroadcastMessage> = {}): BroadcastMessage {
  return {
    type: 'broadcast',
    message_id: 'm1',
    sender_id: 'u1',
    sender: 'Alice',
    sender_avatar: '',
    content: 'bonjour',
    attachment: null,
    reply_to: null,
    recalled_at: null,
    edited_at: null,
    timestamp: '2026-10-01T00:00:00Z',
    favorite_id: null,
    forwarded_from: null,
    reactions: [],
    ...extra,
  }
}

describe('TG-410', () => {
  test('a contact card shows the shared account with its actions', () => {
    const card = message({
      content: '',
      media_kind: 'contact',
      contact: { user_id: 'u9', username: 'carol', display_name: 'Carol', avatar_emoji: '' },
    })
    const html = renderToStaticMarkup(
      <MemoryRouter>
        <ContactContent
          message={card}
          ctx={{
            groupPosition: 'single',
            isOutgoing: false,
            showAvatar: false,
            showSenderName: false,
            highlighted: false,
            selected: false,
          }}
          actions={{}}
          metaSpacer={null}
        />
      </MemoryRouter>,
    )
    expect(html).toContain('Carol')
    expect(html).toContain('@carol')
    expect(html).toContain('发消息')
    expect(html).toContain('添加好友')
  })

  test('«翻译» is hidden while no AI provider is configured, offered once it is', () => {
    const ids = () => buildMessageMenu(message(), {}, { delivered: true }).map((item) => item.id)
    translationStore.setState({ available: false })
    expect(ids()).not.toContain('translate')
    translationStore.setState({ available: true })
    expect(ids()).toContain('translate')
    translationStore.setState({ available: false })
  })
})
