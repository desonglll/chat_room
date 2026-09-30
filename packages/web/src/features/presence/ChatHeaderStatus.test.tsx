// TG-107 markup: the header line renders typing dots + summary, or last-seen/member text.
import { afterEach, expect, test } from 'bun:test'
import { renderToStaticMarkup } from 'react-dom/server'
import type { Chat } from '@tg/core'
import { chatListStore, presenceStore } from '@tg/core'
import { ChatHeaderStatus, ChatTypingLine, TypingIndicator } from './index'

afterEach(() => {
  chatListStore.getState().setChats([])
  presenceStore.getState().clearChat('g')
})

test('TypingIndicator is three decorative dots', () => {
  const html = renderToStaticMarkup(<TypingIndicator action="uploading_photo" />)
  expect(html).toContain('aria-hidden="true"')
  expect(html.match(/tg-typing-dots__dot/g)).toHaveLength(3)
})

test('ChatHeaderStatus shows the member count, then the typing summary with dots', () => {
  chatListStore.getState().setChats([{ id: 'g', chat_type: 'group', title: 'g', member_count: 4 } as unknown as Chat])
  let html = renderToStaticMarkup(<ChatHeaderStatus chatId="g" />)
  expect(html).toContain('4 位成员')
  expect(html).not.toContain('tg-typing-dots')
  expect(renderToStaticMarkup(<ChatTypingLine chatId="g" />)).toBe('')

  presenceStore
    .getState()
    .applyTyping('g', { type: 'typing', content: 'x', action: 'typing', user_id: 'a', username: 'Ann' }, Date.now())
  html = renderToStaticMarkup(<ChatHeaderStatus chatId="g" />)
  expect(html).toContain('data-state="typing"')
  expect(html).toContain('Ann 正在输入')
  expect(html).toContain('tg-typing-dots')
  expect(renderToStaticMarkup(<ChatTypingLine chatId="g" />)).toContain('Ann 正在输入')
})
