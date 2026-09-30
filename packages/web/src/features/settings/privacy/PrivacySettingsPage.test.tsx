// TG-505 markup: the list of dimensions and one dimension's editor, rendered from given rules.
import { expect, test } from 'bun:test'
import { renderToStaticMarkup } from 'react-dom/server'
import { PRIVACY_KEYS, type PrivacyApi, type PrivacyRule } from '@tg/core'
import { PrivacySettingsPage } from './index'

const user = (id: string) => ({ id, username: id, avatar_emoji: '', display_name: `名${id}` })
const rules: PrivacyRule[] = PRIVACY_KEYS.map((key) => ({
  key,
  tier: key === 'last_seen' ? 'contacts' : 'everybody',
  allow_users: key === 'last_seen' ? [user('a')] : [],
  deny_users: key === 'last_seen' ? [user('d')] : [],
}))
const api: PrivacyApi = {
  get: () => Promise.resolve({ rules }),
  put: (_key, _write) => Promise.reject(new Error('not in markup tests')),
  searchUsers: () => Promise.resolve([]),
}

test('the list shows every dimension with its summary and no phone-number row', () => {
  const html = renderToStaticMarkup(<PrivacySettingsPage api={api} initialRules={rules} />)
  for (const title of ['最后上线时间', '头像', '转发消息', '群组邀请', '语音消息']) {
    expect(html).toContain(title)
  }
  expect(html).toContain('我的联系人 (-1, +1)')
  expect(html).not.toContain('手机号')
})

test('the editor shows the tier choice, the reciprocity footnote, and both lists under contacts', () => {
  const html = renderToStaticMarkup(<PrivacySettingsPage api={api} initialRules={rules} initialKey="last_seen" />)
  expect(html).toContain('谁可以看到我的最后上线时间和在线状态？')
  expect(html).toContain('你也将无法看到他人的最后上线时间')
  expect(html).toContain('永不分享给')
  expect(html).toContain('总是分享给')
  expect(html).toContain('名a')
  expect(html).toContain('名d')
  expect(html).toContain('移除 名a')
  expect(html).toMatch(/value="contacts"[^>]*checked|checked[^>]*value="contacts"/)
})

test('under everybody only the never list is offered', () => {
  const html = renderToStaticMarkup(<PrivacySettingsPage api={api} initialRules={rules} initialKey="forwards" />)
  expect(html).toContain('永不允许')
  expect(html).not.toContain('总是允许')
})

test('without initial rules the page starts loading', () => {
  const html = renderToStaticMarkup(<PrivacySettingsPage api={api} />)
  expect(html).toContain('正在加载隐私设置')
  expect(html).toContain('aria-busy="true"')
})
