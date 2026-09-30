/** The archive entry row's markup in each mode (server-rendered; bun test has no DOM). */
import { expect, test } from 'bun:test'
import { renderToStaticMarkup } from 'react-dom/server'
import type { ArchiveRowProps } from './ArchiveRow'
import { ArchiveRow } from './ArchiveRow'
import { ArchivableChatRow } from './ArchivableChatRow'
import { conversation } from './chatListFixtures'

const chats = [
  conversation('a', { title: 'Alpha', unread_count: 2 }),
  conversation('b', { title: 'Beta' }),
  conversation('c', { title: 'Gamma' }),
]

const render = (props: Partial<ArchiveRowProps> = {}) =>
  renderToStaticMarkup(
    <ArchiveRow
      mode="collapsed"
      previewChats={chats}
      count={3}
      badge={{ count: 2, muted: false }}
      now={Date.parse('2026-10-01T12:00:00Z')}
      onOpen={() => {}}
      onModeChange={() => {}}
      {...props}
    />,
  )

test('collapsed: a thin row with the avatar stack, the title and an accent badge', () => {
  const html = render()
  expect(html).toContain('data-mode="collapsed"')
  expect(html.match(/tg-archive__stack-avatar/g)).toHaveLength(3)
  expect(html).toContain('已归档的对话')
  expect(html).toContain('tg-badge--accent')
  expect(html).toContain('3 个会话，2 条未读')
})

test('expanded: the chat names as preview, unread names emphasised', () => {
  const html = render({ mode: 'expanded' })
  expect(html).toContain('data-mode="expanded"')
  expect(html).toContain('<span class="tg-archive__name--unread">Alpha</span>')
  expect(html).toContain('Beta')
  const muted = conversation('m', { title: 'Muted', unread_count: 5 })
  const quiet = render({
    mode: 'expanded',
    previewChats: [{ ...muted, preferences: { ...muted.preferences, notification_level: 'none' } }],
  })
  expect(quiet).not.toContain('tg-archive__name--unread')
  expect(html).not.toContain('tg-archive__stack')
})

test('grey badge when only muted chats have unread; none when nothing is unread', () => {
  expect(render({ badge: { count: 4, muted: true } })).toContain('tg-badge--muted')
  expect(render({ badge: { count: 0, muted: true } })).not.toContain('tg-badge')
})

test('the avatar-only sidebar column shows the archive glyph with the badge', () => {
  const html = render({ collapsed: true })
  expect(html).toContain('tg-chatrow--collapsed')
  expect(html).toContain('tg-archive__glyph-badge')
  expect(html).not.toContain('tg-archive__title')
})

test('an archivable chat row carries the swipe strip labelled for its folder', () => {
  const main = renderToStaticMarkup(
    <ArchivableChatRow archived={false} collapsed={false} onToggleArchive={() => {}}>
      <a href="/chat/a">row</a>
    </ArchivableChatRow>,
  )
  expect(main).toContain('tg-swipe__action--archive')
  expect(main).toContain('>归档<')
  const archived = renderToStaticMarkup(
    <ArchivableChatRow archived collapsed={false} onToggleArchive={() => {}}>
      <a href="/chat/a">row</a>
    </ArchivableChatRow>,
  )
  expect(archived).toContain('tg-swipe__action--unarchive')
  expect(archived).toContain('取消归档')
})
