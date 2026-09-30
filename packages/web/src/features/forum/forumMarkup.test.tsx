// TG-204 markup: topic rows in every state, the topic icon, and the admin switch gate.
import { expect, test } from 'bun:test'
import { renderToStaticMarkup } from 'react-dom/server'
import { MemoryRouter } from 'react-router-dom'
import type { TopicsApi } from '@tg/core'
import { TOPIC_COLORS, topicColorHex } from '@tg/core'
import { ForumToggle } from './ForumToggle'
import { makeTopic } from './topicFixtures'
import { TopicIcon } from './TopicIcon'
import type { TopicRowView } from './topicListModel'
import { TopicRow } from './TopicRow'

const now = new Date('2026-10-01T12:00:00')
const row = (view: TopicRowView) =>
  renderToStaticMarkup(
    <MemoryRouter>
      <ul>
        <TopicRow row={view} href="/chat/c/topic/t" canManage={false} now={now} />
      </ul>
    </MemoryRouter>,
  )

test('a row links to the topic and shows preview, unread badge, lock and mute', () => {
  const html = row({
    dimmed: false,
    topic: makeTopic('t', {
      title: '公告',
      is_closed: true,
      muted: true,
      unread_count: 3,
      last_message: { message_id: 'm', sender: '小明', content: '你好', created_at: '2026-10-01T10:00:00' },
    }),
  })
  expect(html).toContain('href="/chat/c/topic/t"')
  expect(html).toContain('公告')
  expect(html).toContain('小明: </span>你好')
  expect(html).toContain('aria-label="已关闭"')
  expect(html).toContain('aria-label="已静音"')
  expect(html).toContain('data-muted="true"')
  expect(html).toContain('>3<')
})

test('a pinned read row shows the pin; a hidden General is dimmed with a "#" icon', () => {
  expect(row({ dimmed: false, topic: makeTopic('t', { is_pinned: true }) })).toContain('aria-label="已置顶"')
  const general = row({ dimmed: true, topic: makeTopic('g', { is_general: true, is_hidden: true, title: 'General' }) })
  expect(general).toContain('data-dimmed="true"')
  expect(general).toContain('tg-topic-icon--general')
})

test('the icon draws the emoji, else the coloured glyph with the initial', () => {
  expect(renderToStaticMarkup(<TopicIcon title="x" emoji="🔥" color={TOPIC_COLORS[0]} />)).toContain('🔥')
  const glyph = renderToStaticMarkup(<TopicIcon title="游戏" emoji="" color={TOPIC_COLORS[3]} />)
  expect(glyph).toContain(`--topic-color:${topicColorHex(TOPIC_COLORS[3])}`)
  expect(glyph).toContain('>游</text>')
})

test('the forum switch needs chat.info in a group or supergroup', () => {
  const api = {} as TopicsApi
  const render = (type: 'group' | 'channel', perms: string[]) =>
    renderToStaticMarkup(<ForumToggle chatId="c" chatType={type} myPermissions={perms} api={api} initialForum />)
  expect(render('group', ['chat.info'])).toContain('话题')
  expect(render('group', ['message.pin'])).toBe('')
  expect(render('channel', ['chat.info'])).toBe('')
})
