/**
 * Structure over server-rendered markup (`bun test` has no DOM; zustand renders the
 * INITIAL store state server-side, so store-driven parts are fed models directly).
 */
import { expect, test } from 'bun:test'
import { renderToStaticMarkup } from 'react-dom/server'
import { ChatInfoPanel } from '../ChatInfoPanel'
import type { InfoHeaderModel } from '../chatInfoModel'
import { InfoDetails, InfoIdentity } from '../InfoHeader'
import { infoTabs, SharedSection } from '../SharedSection'
import { createChatInfoPagers } from '../useChatInfo'
import type { SharedSources } from '../sharedSources'
import { InfoPane } from '../../shell/InfoPane'
import { fakeClient } from './fixtures'

const group: InfoHeaderModel = {
  variant: 'group',
  title: '设计评审',
  avatarEmoji: '🎨',
  username: '',
  description: '规范见 https://example.com/ds',
  memberCount: 6,
}
const channel: InfoHeaderModel = {
  variant: 'channel',
  title: '公告',
  avatarEmoji: '',
  username: 'news',
  description: '',
  subscriberCount: 12840,
}
const person: InfoHeaderModel = {
  variant: 'private',
  userId: 'u2',
  title: 'Bob 王',
  avatarEmoji: '🦊',
  username: 'bob',
  bio: '设计师',
}

const details = (header: InfoHeaderModel, on = true) =>
  renderToStaticMarkup(
    <InfoDetails
      header={header}
      notificationsOn={on}
      notificationsBusy={false}
      onNotificationsChange={() => undefined}
    />,
  )

test('identity: group shows members (+online), channel subscribers, private the last-seen line', () => {
  const groupHtml = renderToStaticMarkup(<InfoIdentity header={group} onlineCount={3} />)
  expect(groupHtml).toContain('data-variant="group"')
  expect(groupHtml).toContain('6 位成员，3 人在线')
  const channelHtml = renderToStaticMarkup(<InfoIdentity header={channel} />)
  expect(channelHtml).toContain('data-variant="channel"')
  expect(channelHtml).toContain('12,840 位订阅者')
  expect(channelHtml).not.toContain('位成员')
  const privateHtml = renderToStaticMarkup(<InfoIdentity header={person} />)
  expect(privateHtml).toContain('data-variant="private"')
  expect(privateHtml).toContain('Bob 王')
  expect(privateHtml).toContain('tg-chatinfo__status') // useLastSeenText line
})

test('details: bio + @username for a person, linkified description for a group, the notification switch', () => {
  const personHtml = details(person, false)
  expect(personHtml).toContain('设计师')
  expect(personHtml).toContain('@bob')
  expect(personHtml).toContain('用户名')
  expect(personHtml).toContain('role="switch"')
  expect(personHtml).toContain('aria-checked="false"')
  expect(personHtml).toContain('已静音')
  const groupHtml = details(group)
  expect(groupHtml).toContain('href="https://example.com/ds"')
  expect(groupHtml).toContain('rel="noopener noreferrer"')
  expect(groupHtml).not.toContain('用户名')
  expect(groupHtml).toContain('aria-checked="true"')
  expect(details(channel)).toContain('@news')
})

function section(showMembers: boolean) {
  const { client } = fakeClient(() => [])
  const pagers = createChatInfoPagers('c1', client, () => 't', {
    shared: {} as SharedSources,
    loadMembers: async () => [],
  })
  return renderToStaticMarkup(
    <SharedSection chatId="c1" pagers={pagers} showMembers={showMembers} scrollerRef={{ current: null }} />,
  )
}

test('tabs: members first for groups, then the six shared tabs (music since TG-905); only the active panel is mounted', () => {
  expect(infoTabs(true).map((tab) => tab.id)).toEqual(['members', 'media', 'files', 'links', 'music', 'voice', 'gif'])
  expect(infoTabs(false).map((tab) => tab.id)).toEqual(['media', 'files', 'links', 'music', 'voice', 'gif'])
  const html = section(true)
  expect(html).toContain('aria-label="共享内容"')
  expect(html.match(/role="tab"/g)).toHaveLength(7)
  expect(html.match(/role="tabpanel"/g)).toHaveLength(1)
  expect(html).toContain('data-tab="members"')
  const noMembers = section(false)
  expect(noMembers.match(/role="tab"/g)).toHaveLength(6)
  expect(noMembers).toContain('data-tab="media"')
  expect(noMembers).toContain('data-layout="grid"')
})

test('panel shell: close button always, search entry only when wired', () => {
  const { client } = fakeClient(() => [])
  const bare = renderToStaticMarkup(<ChatInfoPanel chatId="c1" client={client} onClose={() => undefined} />)
  expect(bare).toContain('class="tg-info tg-chatinfo"')
  expect(bare).toContain('aria-label="关闭信息面板"')
  expect(bare).not.toContain('在此会话中搜索')
  const searchable = renderToStaticMarkup(
    <ChatInfoPanel chatId="c1" client={client} onClose={() => undefined} onSearchInChat={() => undefined} closing />,
  )
  expect(searchable).toContain('aria-label="在此会话中搜索"')
  expect(searchable).toContain('data-closing="true"')
})

test('InfoPane mounts nothing while the panel is closed', () => {
  expect(renderToStaticMarkup(<InfoPane />)).toBe('')
})
