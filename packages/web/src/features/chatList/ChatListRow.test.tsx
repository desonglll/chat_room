/**
 * The card's "12 states" of a chat row, asserted on server-rendered markup (bun test has
 * no DOM). Each state is one observable marker in the HTML.
 */
import { describe, expect, test } from 'bun:test'
import { renderToStaticMarkup } from 'react-dom/server'
import { ChatListRow } from './ChatListRow'
import type { ChatRowModel } from './chatRowModel'

const model = (overrides: Partial<ChatRowModel> = {}): ChatRowModel => ({
  chatId: 'c1',
  chatType: 'group',
  title: 'Design team',
  avatarLabel: 'Design team',
  avatarInitials: undefined,
  avatarSrc: undefined,
  preview: { kind: 'message', sender: 'alice', media: null, text: 'hello', recalled: false },
  time: '09:05',
  unreadCount: 0,
  muted: false,
  pinned: false,
  outgoing: null,
  showsPresence: false,
  ...overrides,
})

const render = (overrides: Partial<ChatRowModel> = {}, props: Record<string, unknown> = {}) =>
  renderToStaticMarkup(<ChatListRow model={model(overrides)} href="/chat/c1" active={false} {...props} />)

describe('ChatListRow states', () => {
  test('1 avatar + title + time', () => {
    const html = render()
    expect(html).toContain('tg-avatar')
    expect(html).toContain('Design team')
    expect(html).toContain('09:05')
    expect(html).toContain('href="/chat/c1"')
  })

  test('2 online dot only for a private chat reported online', () => {
    expect(render({ chatType: 'private', showsPresence: true }, { isOnline: true })).toContain('tg-avatar__online')
    expect(render({ chatType: 'private', showsPresence: true }, { isOnline: false })).not.toContain('tg-avatar__online')
    expect(render({}, { isOnline: true })).not.toContain('tg-avatar__online')
  })

  test('3 sender prefix', () => {
    expect(render()).toContain('<span class="tg-chatrow__sender">alice: </span>')
  })

  test('4 media glyph with an accessible label', () => {
    const html = render({ preview: { kind: 'message', sender: null, media: 'photo', text: '图片', recalled: false } })
    expect(html).toContain('tg-chatrow__media')
    expect(html).toContain('aria-label="图片"')
  })

  test('5 draft marker', () => {
    const html = render({ preview: { kind: 'draft', text: 'unsent words' } })
    expect(html).toContain('<span class="tg-chatrow__draft">草稿: </span>unsent words')
  })

  test('6 unread badge (accent)', () => {
    const html = render({ unreadCount: 120 })
    expect(html).toContain('tg-badge--accent')
    expect(html).toContain('99+')
    expect(html).toContain('aria-label="120"')
  })

  test('7 muted: bell glyph and a grey badge', () => {
    const html = render({ muted: true, unreadCount: 3 })
    expect(html).toContain('aria-label="已静音"')
    expect(html).toContain('tg-badge--muted')
    expect(html).not.toContain('tg-badge--accent')
  })

  test('8 pinned glyph when nothing is unread; the badge wins otherwise', () => {
    expect(render({ pinned: true })).toContain('aria-label="已置顶"')
    expect(render({ pinned: true, unreadCount: 1 })).not.toContain('aria-label="已置顶"')
  })

  test('9 own last message: single tick sent, double tick read', () => {
    expect(render({ outgoing: 'sent' })).toContain('aria-label="已发送"')
    const read = render({ outgoing: 'read' })
    expect(read).toContain('aria-label="已读"')
    expect(read).toContain('tg-chatrow__ticks--read')
  })

  test('10 typing replaces the preview line', () => {
    const html = render({}, { typingText: 'bob 正在输入…' })
    expect(html).toContain('tg-chatrow__preview--typing')
    expect(html).toContain('bob 正在输入…')
    expect(html).not.toContain('hello')
  })

  test('11 active row is marked for assistive tech and styling', () => {
    const html = renderToStaticMarkup(<ChatListRow model={model()} href="/chat/c1" active />)
    expect(html).toContain('aria-current="page"')
    expect(html).toContain('tg-chatrow--active')
  })

  test('12 collapsed column: avatar only, unread on the avatar, title as tooltip', () => {
    const html = render({ unreadCount: 2 }, { collapsed: true })
    expect(html).toContain('tg-chatrow--collapsed')
    expect(html).toContain('title="Design team"')
    expect(html).toContain('tg-avatar__badge')
    expect(html).not.toContain('tg-chatrow__preview')
  })

  test('recalled and empty previews', () => {
    expect(
      render({ preview: { kind: 'message', sender: null, media: null, text: '消息已撤回', recalled: true } }),
    ).toContain('tg-chatrow__preview--recalled')
    expect(render({ preview: { kind: 'empty', text: '暂无消息' } })).toContain('tg-chatrow__preview--empty')
  })
})
