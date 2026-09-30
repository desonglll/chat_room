/**
 * Server-rendered markup of the row + default renderer: the render contract reaches the
 * DOM (identity only where the context says so, date separator, highlight, system rows).
 */
import { expect, test } from 'bun:test'
import { renderToStaticMarkup } from 'react-dom/server'
import type { BroadcastMessage, MessageLayoutEntry } from '@tg/core'
import { dayLabel } from './dayLabel'
import { renderDefaultMessage } from './DefaultMessage'
import { MessageRow } from './MessageRow'
import type { MessageRenderContext } from './renderContract'

const message: BroadcastMessage = {
  type: 'broadcast',
  message_id: 'm1',
  sender_id: 'u2',
  sender: '小明',
  sender_avatar: '',
  content: '你好',
  attachment: null,
  reply_to: { message_id: 'm0', sender: '小红', content: '早', attachment_file_name: null, recalled: false },
  recalled_at: null,
  edited_at: null,
  timestamp: '2026-09-30T10:00:00Z',
  favorite_id: null,
  forwarded_from: null,
  reactions: [],
}

const entry: MessageLayoutEntry = {
  key: 'm1',
  day: 20_361,
  timeMs: 0,
  joinsPrevious: false,
  startsDay: true,
  groupPosition: 'single',
  isOutgoing: false,
  showAvatar: true,
  showSenderName: true,
}

const ctx = (patch: Partial<MessageRenderContext> = {}): MessageRenderContext => ({
  groupPosition: 'single',
  isOutgoing: false,
  showAvatar: true,
  showSenderName: true,
  highlighted: false,
  selected: false,
  ...patch,
})

test('a row carries its key, date separator and highlight', () => {
  const html = renderToStaticMarkup(
    <MessageRow
      message={message}
      entry={entry}
      dayText="今天"
      highlighted
      selected={false}
      renderMessage={renderDefaultMessage}
    />,
  )
  expect(html).toContain('data-row-key="m1"')
  expect(html).toContain('role="separator"')
  expect(html).toContain('今天')
  expect(html).toContain('data-highlighted="true"')
})

test('renderMessage receives exactly the layout context', () => {
  const seen: MessageRenderContext[] = []
  renderToStaticMarkup(
    <MessageRow
      message={message}
      entry={{ ...entry, groupPosition: 'middle', showAvatar: false, showSenderName: false, startsDay: false }}
      dayText=""
      highlighted={false}
      selected
      renderMessage={(_, context) => {
        seen.push(context)
        return null
      }}
    />,
  )
  expect(seen[0]).toEqual({
    groupPosition: 'middle',
    isOutgoing: false,
    showAvatar: false,
    showSenderName: false,
    highlighted: false,
    selected: true,
  })
})

test('the default renderer shows identity only when asked and makes the quote a jump button', () => {
  const withIdentity = renderToStaticMarkup(renderDefaultMessage(message, ctx()))
  expect(withIdentity).toContain('小明')
  expect(withIdentity).toContain('tg-avatar')
  expect(withIdentity).toContain('<button type="button" class="tg-msg__reply')
  const without = renderToStaticMarkup(renderDefaultMessage(message, ctx({ showAvatar: false, showSenderName: false })))
  expect(without).not.toContain('tg-msg__sender')
  expect(without).not.toContain('tg-avatar')
})

test('system rows render as a centred service pill', () => {
  const html = renderToStaticMarkup(
    renderDefaultMessage({ type: 'system', key: 's', content: '小明 加入了群组' }, ctx()),
  )
  expect(html).toContain('tg-mlist-service__pill')
  expect(html).toContain('小明 加入了群组')
})

test('day labels follow the 今天 / 昨天 / date rule', () => {
  expect(dayLabel(100, 100)).toBe('今天')
  expect(dayLabel(99, 100)).toBe('昨天')
  expect(dayLabel(20_361, 20_400)).toContain('9月30日')
  expect(dayLabel(20_000, 20_400)).toContain('2024年')
  expect(dayLabel(Number.NaN, 1)).toBe('')
})
